use std::cell::Cell;
use std::cell::RefCell;

#[cfg(feature = "mrubyedge-debug")]
use std::env;
use std::rc::Rc;

use crate::Error;
use crate::rite::insn::{Fetched, OpCode};
use crate::yamrb::helpers::mrb_call_inspect;

use super::prelude::hash::mrb_hash_delete;
use super::prelude::object::mrb_object_is_equal;
use super::value::RHashMap;
use super::{helpers::mrb_funcall, value::*, vm::*};

// OpCodes of mruby 3.2.0 from mruby/op.h:
// OPCODE(NOP,        Z)        /* no operation */
// OPCODE(MOVE,       BB)       /* R[a] = R[b] */
// OPCODE(LOADL,      BB)       /* R[a] = Pool[b] */
// OPCODE(LOADI,      BB)       /* R[a] = mrb_int(b) */
// OPCODE(LOADINEG,   BB)       /* R[a] = mrb_int(-b) */
// OPCODE(LOADI__1,   B)        /* R[a] = mrb_int(-1) */
// OPCODE(LOADI_0,    B)        /* R[a] = mrb_int(0) */
// OPCODE(LOADI_1,    B)        /* R[a] = mrb_int(1) */
// OPCODE(LOADI_2,    B)        /* R[a] = mrb_int(2) */
// OPCODE(LOADI_3,    B)        /* R[a] = mrb_int(3) */
// OPCODE(LOADI_4,    B)        /* R[a] = mrb_int(4) */
// OPCODE(LOADI_5,    B)        /* R[a] = mrb_int(5) */
// OPCODE(LOADI_6,    B)        /* R[a] = mrb_int(6) */
// OPCODE(LOADI_7,    B)        /* R[a] = mrb_int(7) */
// OPCODE(LOADI16,    BS)       /* R[a] = mrb_int(b) */
// OPCODE(LOADI32,    BSS)      /* R[a] = mrb_int((b<<16)+c) */
// OPCODE(LOADSYM,    BB)       /* R[a] = Syms[b] */
// OPCODE(LOADNIL,    B)        /* R[a] = nil */
// OPCODE(LOADSELF,   B)        /* R[a] = self */
// OPCODE(LOADT,      B)        /* R[a] = true */
// OPCODE(LOADF,      B)        /* R[a] = false */
// OPCODE(GETGV,      BB)       /* R[a] = getglobal(Syms[b]) */
// OPCODE(SETGV,      BB)       /* setglobal(Syms[b], R[a]) */
// OPCODE(GETSV,      BB)       /* R[a] = Special[Syms[b]] */
// OPCODE(SETSV,      BB)       /* Special[Syms[b]] = R[a] */
// OPCODE(GETIV,      BB)       /* R[a] = ivget(Syms[b]) */
// OPCODE(SETIV,      BB)       /* ivset(Syms[b],R[a]) */
// OPCODE(GETCV,      BB)       /* R[a] = cvget(Syms[b]) */
// OPCODE(SETCV,      BB)       /* cvset(Syms[b],R[a]) */
// OPCODE(GETCONST,   BB)       /* R[a] = constget(Syms[b]) */
// OPCODE(SETCONST,   BB)       /* constset(Syms[b],R[a]) */
// OPCODE(GETMCNST,   BB)       /* R[a] = R[a]::Syms[b] */
// OPCODE(SETMCNST,   BB)       /* R[a+1]::Syms[b] = R[a] */
// OPCODE(GETUPVAR,   BBB)      /* R[a] = uvget(b,c) */
// OPCODE(SETUPVAR,   BBB)      /* uvset(b,c,R[a]) */
// OPCODE(GETIDX,     B)        /* R[a] = R[a][R[a+1]] */
// OPCODE(SETIDX,     B)        /* R[a][R[a+1]] = R[a+2] */
// OPCODE(JMP,        S)        /* pc+=a */
// OPCODE(JMPIF,      BS)       /* if R[a] pc+=b */
// OPCODE(JMPNOT,     BS)       /* if !R[a] pc+=b */
// OPCODE(JMPNIL,     BS)       /* if R[a]==nil pc+=b */
// OPCODE(JMPUW,      S)        /* unwind_and_jump_to(a) */
// OPCODE(EXCEPT,     B)        /* R[a] = exc */
// OPCODE(RESCUE,     BB)       /* R[b] = R[a].isa?(R[b]) */
// OPCODE(RAISEIF,    B)        /* raise(R[a]) if R[a] */
// OPCODE(SSEND,      BBB)      /* R[a] = self.send(Syms[b],R[a+1]..,R[a+n+1]:R[a+n+2]..) (c=n|k<<4) */
// OPCODE(SSENDB,     BBB)      /* R[a] = self.send(Syms[b],R[a+1]..,R[a+n+1]:R[a+n+2]..,&R[a+n+2k+1]) */
// OPCODE(SEND,       BBB)      /* R[a] = R[a].send(Syms[b],R[a+1]..,R[a+n+1]:R[a+n+2]..) (c=n|k<<4) */
// OPCODE(SENDB,      BBB)      /* R[a] = R[a].send(Syms[b],R[a+1]..,R[a+n+1]:R[a+n+2]..,&R[a+n+2k+1]) */
// OPCODE(CALL,       Z)        /* self.call(*, **, &) (But overlay the current call frame; tailcall) */
// OPCODE(SUPER,      BB)       /* R[a] = super(R[a+1],... ,R[a+b+1]) */
// OPCODE(ARGARY,     BS)       /* R[a] = argument array (16=m5:r1:m5:d1:lv4) */
// OPCODE(ENTER,      W)        /* arg setup according to flags (23=m5:o5:r1:m5:k5:d1:b1) */
// OPCODE(KEY_P,      BB)       /* R[a] = kdict.key?(Syms[b]) */
// OPCODE(KEYEND,     Z)        /* raise unless kdict.empty? */
// OPCODE(KARG,       BB)       /* R[a] = kdict[Syms[b]]; kdict.delete(Syms[b]) */
// OPCODE(RETURN,     B)        /* return R[a] (normal) */
// OPCODE(RETURN_BLK, B)        /* return R[a] (in-block return) */
// OPCODE(BREAK,      B)        /* break R[a] */
// OPCODE(BLKPUSH,    BS)       /* R[a] = block (16=m5:r1:m5:d1:lv4) */
// OPCODE(ADD,        B)        /* R[a] = R[a]+R[a+1] */
// OPCODE(ADDI,       BB)       /* R[a] = R[a]+mrb_int(b) */
// OPCODE(SUB,        B)        /* R[a] = R[a]-R[a+1] */
// OPCODE(SUBI,       BB)       /* R[a] = R[a]-mrb_int(b) */
// OPCODE(MUL,        B)        /* R[a] = R[a]*R[a+1] */
// OPCODE(DIV,        B)        /* R[a] = R[a]/R[a+1] */
// OPCODE(EQ,         B)        /* R[a] = R[a]==R[a+1] */
// OPCODE(LT,         B)        /* R[a] = R[a]<R[a+1] */
// OPCODE(LE,         B)        /* R[a] = R[a]<=R[a+1] */
// OPCODE(GT,         B)        /* R[a] = R[a]>R[a+1] */
// OPCODE(GE,         B)        /* R[a] = R[a]>=R[a+1] */
// OPCODE(ARRAY,      BB)       /* R[a] = ary_new(R[a],R[a+1]..R[a+b]) */
// OPCODE(ARRAY2,     BBB)      /* R[a] = ary_new(R[b],R[b+1]..R[b+c]) */
// OPCODE(ARYCAT,     B)        /* ary_cat(R[a],R[a+1]) */
// OPCODE(ARYPUSH,    BB)       /* ary_push(R[a],R[a+1]..R[a+b]) */
// OPCODE(ARYSPLAT,   B)        /* R[a] = ary_splat(R[a]) */
// OPCODE(AREF,       BBB)      /* R[a] = R[b][c] */
// OPCODE(ASET,       BBB)      /* R[b][c] = R[a] */
// OPCODE(APOST,      BBB)      /* *R[a],R[a+1]..R[a+c] = R[a][b..] */
// OPCODE(INTERN,     B)        /* R[a] = intern(R[a]) */
// OPCODE(SYMBOL,     BB)       /* R[a] = intern(Pool[b]) */
// OPCODE(STRING,     BB)       /* R[a] = str_dup(Pool[b]) */
// OPCODE(STRCAT,     B)        /* str_cat(R[a],R[a+1]) */
// OPCODE(HASH,       BB)       /* R[a] = hash_new(R[a],R[a+1]..R[a+b*2-1]) */
// OPCODE(HASHADD,    BB)       /* hash_push(R[a],R[a+1]..R[a+b*2]) */
// OPCODE(HASHCAT,    B)        /* R[a] = hash_cat(R[a],R[a+1]) */
// OPCODE(LAMBDA,     BB)       /* R[a] = lambda(Irep[b],L_LAMBDA) */
// OPCODE(BLOCK,      BB)       /* R[a] = lambda(Irep[b],L_BLOCK) */
// OPCODE(METHOD,     BB)       /* R[a] = lambda(Irep[b],L_METHOD) */
// OPCODE(RANGE_INC,  B)        /* R[a] = range_new(R[a],R[a+1],FALSE) */
// OPCODE(RANGE_EXC,  B)        /* R[a] = range_new(R[a],R[a+1],TRUE) */
// OPCODE(OCLASS,     B)        /* R[a] = ::Object */
// OPCODE(CLASS,      BB)       /* R[a] = newclass(R[a],Syms[b],R[a+1]) */
// OPCODE(MODULE,     BB)       /* R[a] = newmodule(R[a],Syms[b]) */
// OPCODE(EXEC,       BB)       /* R[a] = blockexec(R[a],Irep[b]) */
// OPCODE(DEF,        BB)       /* R[a].newmethod(Syms[b],R[a+1]); R[a] = Syms[b] */
// OPCODE(ALIAS,      BB)       /* alias_method(target_class,Syms[a],Syms[b]) */
// OPCODE(UNDEF,      B)        /* undef_method(target_class,Syms[a]) */
// OPCODE(SCLASS,     B)        /* R[a] = R[a].singleton_class */
// OPCODE(TCLASS,     B)        /* R[a] = target_class */
// OPCODE(DEBUG,      BBB)      /* print a,b,c */
// OPCODE(ERR,        B)        /* raise(LocalJumpError, Pool[a]) */
// OPCODE(EXT1,       Z)        /* make 1st operand (a) 16bit */
// OPCODE(EXT2,       Z)        /* make 2nd operand (b) 16bit */
// OPCODE(EXT3,       Z)        /* make 1st and 2nd operands 16bit */
// OPCODE(STOP,       Z)        /* stop VM */
// OpCodes added or changed in mruby 4.0 (RITE0400):
// OPCODE(LOADTRUE,   B)        /* R[a] = true (LOADT in 3.x) */
// OPCODE(LOADFALSE,  B)        /* R[a] = false (LOADF in 3.x) */
// OPCODE(GETIDX0,    BB)       /* R[a] = R[b][0]; a+1 for method call */
// OPCODE(MATCHERR,   B)        /* raise NoMatchingPatternError unless R[a] */
// OPCODE(SSEND0,     BB)       /* R[a] = self.send(Syms[b]) (no args) */
// OPCODE(SEND0,      BB)       /* R[a] = R[a].send(Syms[b]) (no args) */
// OPCODE(BLKCALL,    BB)       /* R[a] = R[a].call(R[a+1],... ,R[a+b]); direct block call */
// OPCODE(ENTER,      W)        /* arg setup according to flags (24=n1:m5:o5:r1:m5:k5:d1:b1) */
// OPCODE(RETSELF,    Z)        /* return self */
// OPCODE(RETNIL,     Z)        /* return nil */
// OPCODE(RETTRUE,    Z)        /* return true */
// OPCODE(RETFALSE,   Z)        /* return false */
// OPCODE(ADDILV,     BBB)      /* R[a] = R[a]+mrb_int(c); R[b],R[b+1] for method call */
// OPCODE(SUBILV,     BBB)      /* R[a] = R[a]-mrb_int(c); R[b],R[b+1] for method call */
// OPCODE(TDEF,       BBB)      /* target_class.newmethod(Syms[b],Irep[c]); R[a] = Syms[b] */
// OPCODE(SDEF,       BBB)      /* R[a].singleton_class.newmethod(Syms[b],Irep[c]); R[a] = Syms[b] */
// functions that represent each opcode are defined in this file.
// to understand the meaning of each operand mark, see enum Fetched in rite/insn.rs:
// pub enum Fetched {
//     Z,
//     B(u8),
//     BB(u8, u8),
//     BBB(u8, u8, u8),
//     BS(u8, u16),
//     BSS(u8, u16, u16),
//     S(u16),
//     W(u32), // u24 in real layout
// }
//

const ENTER_N1_MASK: u32 = 0b1 << 23;
const ENTER_M1_MASK: u32 = 0b11111 << 18;
const ENTER_O_MASK: u32 = 0b11111 << 13;
const ENTER_R_MASK: u32 = 0b1 << 12;
const ENTER_M2_MASK: u32 = 0b11111 << 7;
const ENTER_K_MASK: u32 = 0b11111 << 2;
const ENTER_D_MASK: u32 = 0b1 << 1;
const ENTER_B_MASK: u32 = 0b1 << 0;

#[inline(always)]
pub(crate) fn consume_expr(
    vm: &mut VM,
    code: OpCode,
    operand: &Fetched,
    pos: usize,
    len: usize,
) -> Result<(), Error> {
    use crate::rite::insn::OpCode::*;
    match code {
        NOP => {
            op_nop(vm, operand)?;
        }
        MOVE => {
            op_move(vm, operand)?;
        }
        LOADL => {
            op_loadl(vm, operand)?;
        }
        LOADI => {
            op_loadi(vm, operand)?;
        }
        LOADINEG => {
            op_loadineg(vm, operand)?;
        }
        LOADI__1 => {
            op_loadi_n(vm, -1, operand)?;
        }
        LOADI_0 => {
            op_loadi_n(vm, 0, operand)?;
        }
        LOADI_1 => {
            op_loadi_n(vm, 1, operand)?;
        }
        LOADI_2 => {
            op_loadi_n(vm, 2, operand)?;
        }
        LOADI_3 => {
            op_loadi_n(vm, 3, operand)?;
        }
        LOADI_4 => {
            op_loadi_n(vm, 4, operand)?;
        }
        LOADI_5 => {
            op_loadi_n(vm, 5, operand)?;
        }
        LOADI_6 => {
            op_loadi_n(vm, 6, operand)?;
        }
        LOADI_7 => {
            op_loadi_n(vm, 7, operand)?;
        }
        LOADI16 => {
            op_loadi16(vm, operand)?;
        }
        LOADI32 => {
            op_loadi32(vm, operand)?;
        }
        LOADSYM => {
            op_loadsym(vm, operand)?;
        }
        LOADNIL => {
            op_loadnil(vm, operand)?;
        }
        LOADSELF => {
            op_loadself(vm, operand)?;
        }
        LOADT => {
            op_loadt(vm, operand)?;
        }
        LOADF => {
            op_loadf(vm, operand)?;
        }
        GETGV => {
            op_getgv(vm, operand)?;
        }
        SETGV => {
            op_setgv(vm, operand)?;
        }
        // GETSV => {
        //     // op_getsv(vm, &operand)?;
        // }
        // SETSV => {
        //     // op_setsv(vm, &operand)?;
        // }
        GETIV => {
            op_getiv(vm, operand)?;
        }
        SETIV => {
            op_setiv(vm, operand)?;
        }
        GETCV => {
            op_getcv(vm, operand)?;
        }
        SETCV => {
            op_setcv(vm, operand)?;
        }
        GETCONST => {
            op_getconst(vm, operand)?;
        }
        SETCONST => {
            op_setconst(vm, operand)?;
        }
        GETMCNST => {
            op_getmcnst(vm, operand)?;
        }
        SETMCNST => {
            op_setmcnst(vm, operand)?;
        }
        GETUPVAR => {
            op_getupvar(vm, operand)?;
        }
        SETUPVAR => {
            op_setupvar(vm, operand)?;
        }
        GETIDX => {
            op_getidx(vm, operand)?;
        }
        SETIDX => {
            op_setidx(vm, operand)?;
        }
        JMP => {
            op_jmp(vm, operand, pos + len)?;
        }
        JMPIF => {
            op_jmpif(vm, operand, pos + len)?;
        }
        JMPNOT => {
            op_jmpnot(vm, operand, pos + len)?;
        }
        JMPNIL => {
            op_jmpnil(vm, operand, pos + len)?;
        }
        JMPUW => {
            op_jmpuw(vm, operand, pos + len)?;
        }
        EXCEPT => {
            op_except(vm, operand)?;
        }
        RESCUE => {
            op_rescue(vm, operand)?;
        }
        RAISEIF => {
            op_raiseif(vm, operand)?;
        }
        SSEND => {
            op_ssend(vm, operand)?;
        }
        SSENDB => {
            op_ssendb(vm, operand)?;
        }
        SEND => {
            op_send(vm, operand)?;
        }
        SENDB => {
            op_sendb(vm, operand)?;
        }
        CALL => {
            op_call(vm, operand)?;
        }
        SUPER => {
            op_super(vm, operand)?;
        }
        ARGARY => {
            op_argary(vm, operand)?;
        }
        ENTER => {
            op_enter(vm, operand)?;
        }
        KEY_P => {
            op_key_p(vm, operand)?;
        }
        KEYEND => {
            op_keyend(vm, operand)?;
        }
        KARG => {
            op_karg(vm, operand)?;
        }
        RETURN => {
            op_return(vm, operand)?;
        }
        RETURN_BLK => {
            op_return_blk(vm, operand)?;
        }
        BREAK => {
            op_break(vm, operand)?;
        }
        BLKPUSH => {
            op_blkpush(vm, operand)?;
        }
        ADD => {
            op_add(vm, operand)?;
        }
        ADDI => {
            op_addi(vm, operand)?;
        }
        SUB => {
            op_sub(vm, operand)?;
        }
        SUBI => {
            op_subi(vm, operand)?;
        }
        MUL => {
            op_mul(vm, operand)?;
        }
        DIV => {
            op_div(vm, operand)?;
        }
        EQ => {
            op_eq(vm, operand)?;
        }
        LT => {
            op_lt(vm, operand)?;
        }
        LE => {
            op_le(vm, operand)?;
        }
        GT => {
            op_gt(vm, operand)?;
        }
        GE => {
            op_ge(vm, operand)?;
        }
        ARRAY => {
            op_array(vm, operand)?;
        }
        ARRAY2 => {
            op_array2(vm, operand)?;
        }
        ARYCAT => {
            op_arycat(vm, operand)?;
        }
        ARYPUSH => {
            op_arypush(vm, operand)?;
        }
        ARYSPLAT => {
            op_arysplat(vm, operand)?;
        }
        AREF => {
            op_aref(vm, operand)?;
        }
        // ASET => {
        //     // op_aset(vm, &operand)?;
        // }
        APOST => {
            op_apost(vm, operand)?;
        }
        // INTERN => {
        //     // op_intern(vm, &operand)?;
        // }
        SYMBOL => {
            op_symbol(vm, operand)?;
        }
        STRING => {
            op_string(vm, operand)?;
        }
        STRCAT => {
            op_strcat(vm, operand)?;
        }
        HASH => {
            op_hash(vm, operand)?;
        }
        HASHADD => {
            op_hashadd(vm, operand)?;
        }
        HASHCAT => {
            op_hashcat(vm, operand)?;
        }
        LAMBDA => {
            op_lambda(vm, operand)?;
        }
        BLOCK => {
            op_block(vm, operand)?;
        }
        METHOD => {
            op_method(vm, operand)?;
        }
        RANGE_INC => {
            op_range_inc(vm, operand)?;
        }
        RANGE_EXC => {
            op_range_exc(vm, operand)?;
        }
        OCLASS => {
            op_oclass(vm, operand)?;
        }
        CLASS => {
            op_class(vm, operand)?;
        }
        MODULE => {
            op_module(vm, operand)?;
        }
        EXEC => {
            op_exec(vm, operand)?;
        }
        DEF => {
            op_def(vm, operand)?;
        }
        ALIAS => {
            op_alias(vm, operand)?;
        }
        UNDEF => {
            op_undef(vm, operand)?;
        }
        SCLASS => {
            op_sclass(vm, operand)?;
        }
        TCLASS => {
            op_tclass(vm, operand)?;
        }
        // DEBUG => {
        //     // op_debug(vm, &operand)?;
        // }
        // ERR => {
        //     // op_err(vm, &operand)?;
        // }
        // EXT1 => {
        //     // op_ext1(vm, &operand)?;
        // }
        // EXT2 => {
        //     // op_ext2(vm, &operand)?;
        // }
        // EXT3 => {
        //     // op_ext3(vm, &operand)?;
        // }
        STOP => {
            op_stop(vm, operand)?;
        }
        // mruby 4.0 (RITE0400)
        GETIDX0 => {
            op_getidx0(vm, operand)?;
        }
        MATCHERR => {
            op_matcherr(vm, operand)?;
        }
        SSEND0 => {
            op_ssend0(vm, operand)?;
        }
        SEND0 => {
            op_send0(vm, operand)?;
        }
        TDEF => {
            op_tdef(vm, operand)?;
        }
        SDEF => {
            op_sdef(vm, operand)?;
        }
        BLKCALL => {
            op_blkcall(vm, operand)?;
        }
        RETSELF => {
            op_retself(vm, operand)?;
        }
        RETNIL => {
            op_retnil(vm, operand)?;
        }
        RETTRUE => {
            op_rettrue(vm, operand)?;
        }
        RETFALSE => {
            op_retfalse(vm, operand)?;
        }
        ADDILV => {
            op_addilv(vm, operand)?;
        }
        SUBILV => {
            op_subilv(vm, operand)?;
        }
        _ => {
            unimplemented!("{:?}: Not supported yet", code)
        }
    }
    Ok(())
}

pub(crate) fn push_callinfo(
    vm: &mut VM,
    method_id: u32,
    n_args: usize,
    method_owner: Option<Rc<RModule>>,
    return_reg: usize,
    is_funcall: bool,
) {
    vm.current_n_args.set(n_args);
    vm.callinfo_stack.push(CALLINFO {
        method_id,
        pc_irep: vm.current_irep.clone(),
        pc: vm.pc.get(),
        current_regs_offset: vm.current_regs_offset,
        n_args,
        return_reg,
        target_class: vm.target_class.clone(),
        method_owner,
        has_block: Cell::new(false),
        kargs_pushed: Cell::new(false),
        is_funcall,
    });
}

fn calcurate_pc(irep: &IREP, pc: usize, original_pc: usize) -> usize {
    let mut next_pc = pc;
    loop {
        let op = irep.code.get(next_pc).expect("cannot fetch op anymore");
        if op.pos == original_pc {
            break;
        }
        next_pc += 1;
    }
    next_pc
}

pub(crate) fn op_nop(_vm: &mut VM, _operand: &Fetched) -> Result<(), Error> {
    // NOOP
    Ok(())
}

pub(crate) fn op_loadi_n(vm: &mut VM, n: i32, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    vm.current_regs()[a].replace(Value::Integer(n as i64));
    Ok(())
}

pub(crate) fn op_loadl(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let pool_val = vm.current_irep.pool[b as usize].clone();
    let val = match pool_val {
        RPool::Str(s) => Value::Object(Rc::new(RObject::string(s))),
        RPool::Int(i) => Value::Integer(i),
        RPool::Float(f) => Value::Float(f),
        RPool::Data(_) => {
            return Err(Error::Internal(
                "Binary data in pool not supported yet".to_string(),
            ));
        }
    };
    vm.current_regs()[a as usize].replace(val);
    Ok(())
}

pub(crate) fn op_loadi16(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bs()?;
    vm.current_regs()[a as usize].replace(Value::Integer(b as i64));
    Ok(())
}

pub(crate) fn op_loadi32(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bss()?;
    vm.current_regs()[a as usize].replace(Value::Integer((b as i64) << 16 | c as i64));
    Ok(())
}

pub(crate) fn op_loadi(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    vm.current_regs()[a as usize].replace(Value::Integer(b as i64));
    Ok(())
}

pub(crate) fn op_loadineg(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    vm.current_regs()[a as usize].replace(Value::Integer(-(b as i64)));
    Ok(())
}

pub(crate) fn op_loadsym(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let sym = vm.current_irep.syms[b as usize].clone();
    vm.current_regs()[a as usize].replace(Value::Symbol(sym.id));
    Ok(())
}

pub(crate) fn op_loadnil(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    vm.current_regs()[a].replace(Value::Nil);
    Ok(())
}

pub(crate) fn op_loadself(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let regs = vm.current_regs();
    let val = regs[0]
        .clone()
        .ok_or_else(|| Error::internal("self is not assigned"))?;
    regs[a].replace(val);
    Ok(())
}

pub(crate) fn op_loadt(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    vm.current_regs()[a].replace(Value::Bool(true));
    Ok(())
}

pub(crate) fn op_loadf(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    vm.current_regs()[a].replace(Value::Bool(false));
    Ok(())
}

pub(crate) fn op_getgv(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    // Borrow the sym name instead of cloning it; globals hashes it
    // transiently and never stores the key by value.
    let name = &vm.current_irep.syms[b as usize].name;
    // Ruby reads a global that was never assigned as nil.
    let val = match vm.globals.get(name) {
        Some(val) => val.clone(),
        None => Value::Nil,
    };
    vm.set_reg_value(a as usize, val);
    Ok(())
}

pub(crate) fn op_setgv(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let val = vm.get_reg_value(a as usize);
    // Clone only the key; the intermediate RSym clone was redundant.
    vm.globals
        .insert(vm.current_irep.syms[b as usize].name.clone(), val);
    Ok(())
}

pub(crate) fn op_getiv(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let id = vm.current_irep.syms[b as usize].id;
    // Read the ivar through the receiver borrowed in place (no self Rc clone);
    // only a non-object self (rare) falls back to boxing.
    let value = match &vm.regs[vm.current_regs_offset] {
        Some(Value::Object(o)) => o.get_ivar_by_id(id),
        Some(v) => v.to_rc().get_ivar_by_id(id),
        None => return Err(Error::internal("self is not assigned")),
    };
    vm.current_regs()[a as usize].replace(value);
    Ok(())
}

pub(crate) fn op_setiv(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let id = vm.current_irep.syms[b as usize].id;
    let val = vm.current_regs()[a as usize]
        .clone()
        .ok_or_else(|| Error::internal(format!("register {} is not assigned", a)))?;
    // immediates are shared, so writing an ivar on one would
    // leak to every instance; MRI forbids it with FrozenError.
    match &vm.regs[vm.current_regs_offset] {
        Some(Value::Object(o)) => o.set_ivar_by_id(id, val),
        Some(v) => return Err(v.to_rc().frozen_immediate_error(vm)),
        None => return Err(Error::internal("self is not assigned")),
    }
    Ok(())
}

// Quirk (documented): class variables are stored in the canonical class
// object's ivar table (mirroring mruby, which keeps cvars in RClass.iv).
// Resolution walks the superclass chain of the class of self, never the
// metaclass: mruby resolves class(self) which for a class body self is the
// singleton, but the observable result is the same for normal use.
pub(crate) fn op_getcv(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let name = vm.current_irep.syms[b as usize].name.clone();
    let value = cvar_lookup(vm, &name)?;
    vm.set_reg(a as usize, value);
    Ok(())
}

// Quirk (documented): assignment walks the chain and overwrites the cvar at
// its definition site when an ancestor defines it, so subclasses and parent
// share one value; otherwise it is stored on the class of self.
pub(crate) fn op_setcv(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let name = vm.current_irep.syms[b as usize].name.clone();
    let value = vm.get_current_regs_cloned(a as usize)?;
    cvar_set(vm, &name, value);
    Ok(())
}

/// Class context for cvar resolution: the class whose body runs (self is a
/// Class) or the runtime class of self inside an instance method.
fn class_context(vm: &mut VM) -> Result<Rc<RClass>, Error> {
    let obj = vm.current_regs()[0]
        .clone()
        .ok_or_else(|| Error::internal("self is not assigned"))?
        .to_rc();
    match &obj.value {
        RValue::Class(klass) => Ok(klass.clone()),
        // Quirk (documented): module-level cvars are unsupported — the
        // module wrapper is recreated on each RObject::module() call, so
        // there is no stable table to persist into.
        RValue::Module(_) => Err(Error::TypeMismatch),
        _ => Ok(obj.get_class(vm)),
    }
}

fn cvar_lookup(vm: &mut VM, name: &str) -> Result<Rc<RObject>, Error> {
    let mut current: Option<Rc<RClass>> = Some(class_context(vm)?);
    while let Some(klass) = current.clone() {
        let wrapper = RObject::class(klass.clone(), vm);
        if let Some(val) = wrapper.ivar.borrow().get(intern_symbol(name)).cloned() {
            return Ok(val.to_rc());
        }
        current = klass.super_class.clone();
    }
    Err(Error::NameError(format!(
        "uninitialized class variable {name}"
    )))
}

fn cvar_set(vm: &mut VM, name: &str, value: Rc<RObject>) {
    let cls = match class_context(vm) {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut current = Some(cls.clone());
    while let Some(klass) = current.clone() {
        let wrapper = RObject::class(klass.clone(), vm);
        let key = intern_symbol(name);
        if wrapper.ivar.borrow().contains_key(key) {
            wrapper.ivar.borrow_mut().insert(key, Value::from_rc(value));
            return;
        }
        current = klass.super_class.clone();
    }
    let wrapper = RObject::class(cls, vm);
    wrapper
        .ivar
        .borrow_mut()
        .insert(intern_symbol(name), Value::from_rc(value));
}

pub(crate) fn op_getconst(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let irep = vm.current_irep.clone();
    let name = &irep.syms[b as usize].name;

    // Inline constant cache: the site is the current pc (the loop advanced
    // past this instruction). A hit needs both the constant-table version
    // (guards redefinition) and the resolving namespace identity (guards the
    // same instruction seeing different lexical scopes).
    let site = vm.pc.get() - 1;
    let version = vm.const_version.get();
    let ns = current_namespace(vm).or_else(|| {
        let obj = vm.current_regs()[0].clone();
        obj.as_ref().map(|o| o.get_class(vm).module.clone())
    });
    let cached = {
        let caches = vm.current_irep.const_cache.borrow();
        caches
            .get(site)
            .and_then(|slot| slot.as_ref())
            .filter(|entry| {
                entry.version == version
                    && entry.ns.as_ref().map(Rc::as_ptr) == ns.as_ref().map(Rc::as_ptr)
            })
            .map(|entry| entry.value.clone())
    };
    if let Some(value) = cached {
        vm.set_reg_value(a as usize, value);
        return Ok(());
    }

    // Miss: walk the namespace chain upwards until found, then the global
    // table.
    let mut resolved: Option<Value> = None;
    let mut current = ns.clone();
    while let Some(cur) = current.clone() {
        if let Some(val) = cur.consts.borrow().get(name).cloned() {
            resolved = Some(Value::from_rc(val));
            break;
        }
        current = cur.parent.borrow().clone();
    }
    if resolved.is_none()
        && let Some(val) = vm.consts.get(name).cloned()
    {
        resolved = Some(Value::from_rc(val));
    }
    let value = resolved.ok_or_else(|| Error::NameError(name.clone()))?;

    if let Some(slot) = vm.current_irep.const_cache.borrow_mut().get_mut(site) {
        *slot = Some(ConstCacheEntry {
            version,
            ns,
            value: value.clone(),
        });
    }
    vm.set_reg_value(a as usize, value);
    Ok(())
}

pub(crate) fn op_setconst(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let name = vm.current_irep.syms[b as usize].name.clone();
    let val = vm.get_current_regs_cloned(a as usize)?;
    // Scope the constant to the defining module/class so qualified reads
    // (Module::CONST via op_getmcnst) resolve. Without a cref, assignments
    // inside an instance method fall back to the global table.
    match current_namespace(vm) {
        Some(ns) => {
            ns.consts.borrow_mut().insert(name, val);
        }
        None => {
            // Top-level constants also live in Object's table (define_class/
            // define_module mirror there); keep both in sync so reads that
            // resolve through the class of self agree.
            vm.consts.insert(name.clone(), val.clone());
            vm.object_class.consts.borrow_mut().insert(name, val);
        }
    }
    vm.bump_const_version();
    Ok(())
}

pub(crate) fn op_getmcnst(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let recv = vm.get_reg_value(a as usize);
    let irep = vm.current_irep.clone();
    let name = &irep.syms[b as usize].name;
    let module = recv.rvalue().and_then(|rv| match rv {
        RValue::Class(klass) => Some(klass.module.clone()),
        RValue::Module(module) => Some(module.clone()),
        _ => None,
    });

    // Inline constant cache keyed by the receiver module: qualified reads are
    // independent of the lexical scope, so only the receiver and the constant
    // version identify the resolution.
    let site = vm.pc.get() - 1;
    let version = vm.const_version.get();
    let cached = {
        let caches = vm.current_irep.const_cache.borrow();
        caches
            .get(site)
            .and_then(|slot| slot.as_ref())
            .filter(|entry| {
                entry.version == version
                    && entry.ns.as_ref().map(Rc::as_ptr) == module.as_ref().map(Rc::as_ptr)
            })
            .map(|entry| entry.value.clone())
    };
    if let Some(value) = cached {
        vm.set_reg_value(a as usize, value);
        return Ok(());
    }

    let mut current = module.clone();
    let mut resolved: Option<Value> = None;
    while let Some(cur) = current.clone() {
        if let Some(val) = cur.consts.borrow().get(name).cloned() {
            resolved = Some(Value::from_rc(val));
            break;
        }
        current = cur.parent.borrow().clone();
    }
    let value = resolved.ok_or_else(|| Error::NameError(name.clone()))?;

    if let Some(slot) = vm.current_irep.const_cache.borrow_mut().get_mut(site) {
        *slot = Some(ConstCacheEntry {
            version,
            ns: module,
            value: value.clone(),
        });
    }
    vm.set_reg_value(a as usize, value);
    Ok(())
}

// Quirk (documented): operand layout is value in R[a], module in R[a+1]
// (mruby: mrb_const_set(R[a+1], Syms[b], R[a])). The top-level `::B = v`
// case resolves the module to Object's class, whose module consts are read
// by bare GETCONST through the class-of-self fallback.
pub(crate) fn op_setmcnst(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let name = vm.current_irep.syms[b as usize].name.clone();
    let module = vm.get_current_regs_cloned(a as usize + 1)?;
    let value = vm.get_current_regs_cloned(a as usize)?;
    match &module.value {
        RValue::Class(klass) => {
            klass.module.consts.borrow_mut().insert(name, value);
        }
        RValue::Module(module) => {
            module.consts.borrow_mut().insert(name, value);
        }
        _ => {
            return Err(Error::TypeMismatch);
        }
    }
    vm.bump_const_version();
    Ok(())
}

pub(crate) fn op_getupvar(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;
    let n = c as usize;
    let mut environ = vm
        .upper
        .as_ref()
        .ok_or_else(|| Error::internal("op_getupvar expects upper env"))?;
    for _ in 0..n {
        environ = environ
            .upper
            .as_ref()
            .ok_or_else(|| Error::internal("op_getupvar failed to find upvar"))?;
    }
    let environ = environ.clone();
    let up_regs = &vm.regs[environ.current_regs_offset..];
    if !environ.expired() {
        if let Some(val) = up_regs[b as usize].as_ref().cloned() {
            vm.current_regs()[a as usize].replace(val);
        } else {
            return Err(Error::internal(format!("register {} is empty", b)));
        }
    } else {
        let captured = environ.captured.borrow();
        let val = &captured
            .as_ref()
            .ok_or_else(|| Error::internal("captured environment not found"))?[b as usize];
        let val = val.clone();
        vm.current_regs()[a as usize].replace(Value::from_rc(
            val.ok_or_else(|| Error::internal("captured value not found"))?,
        ));
    }
    Ok(())
}

pub(crate) fn op_setupvar(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;
    let n = c as usize;
    let mut environ = vm
        .upper
        .as_ref()
        .ok_or_else(|| Error::internal("op_getupvar expects upper env"))?;
    for _ in 0..n {
        environ = environ
            .upper
            .as_ref()
            .ok_or_else(|| Error::internal("op_getupvar failed to find upvar"))?;
    }
    let environ = environ.clone();
    let current_regs_offset = environ.current_regs_offset;

    let val = vm.current_regs()[a as usize]
        .clone()
        .ok_or_else(|| Error::internal(format!("register {} is not assigned", a)))?;
    if !environ.expired() {
        let up_regs = &mut vm.regs[current_regs_offset..];
        let target = &mut up_regs[b as usize];
        target.replace(val);
    } else {
        let mut captured = environ.captured.borrow_mut();
        let captured = captured
            .as_mut()
            .ok_or_else(|| Error::internal("captured environment not found"))?;
        let target = &mut captured[b as usize];
        target.replace(val.to_rc());
    }
    Ok(())
}

/// Whether the GETIDX/SETIDX fast path is safe for this receiver: it must be
/// a plain Array (not a subclass) whose `[]` still resolves to the pristine
/// builtin. The verdict is cached and reset on every method-version bump, so
/// a redefined Array#[] immediately falls back to dispatch.
fn array_index_fast(vm: &VM, recv: &Value) -> bool {
    if !Rc::ptr_eq(&recv.get_class(vm), &vm.array_class) {
        return false;
    }
    match vm.array_fast.get() {
        Some(verdict) => verdict,
        None => {
            let pristine = vm
                .resolve_method_cached(&vm.array_class, "[]")
                .and_then(|(_, m)| m.func)
                == vm.array_index_func.get();
            vm.array_fast.set(Some(pristine));
            pristine
        }
    }
}

fn hash_index_fast(vm: &VM, recv: &Value) -> bool {
    if !Rc::ptr_eq(&recv.get_class(vm), &vm.hash_class) {
        return false;
    }
    match vm.hash_index_fast.get() {
        Some(verdict) => verdict,
        None => {
            let pristine = vm
                .resolve_method_cached(&vm.hash_class, "[]")
                .and_then(|(_, m)| m.func)
                == vm.hash_index_func.get();
            vm.hash_index_fast.set(Some(pristine));
            pristine
        }
    }
}

fn hash_aset_fast(vm: &VM, recv: &Value) -> bool {
    if !Rc::ptr_eq(&recv.get_class(vm), &vm.hash_class) {
        return false;
    }
    match vm.hash_aset_fast.get() {
        Some(verdict) => verdict,
        None => {
            let pristine = vm
                .resolve_method_cached(&vm.hash_class, "[]=")
                .and_then(|(_, m)| m.func)
                == vm.hash_aset_func.get();
            vm.hash_aset_fast.set(Some(pristine));
            pristine
        }
    }
}

pub(crate) fn op_getidx(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let recv = vm.current_regs()[a]
        .clone()
        .ok_or_else(|| Error::internal(format!("register {} is not assigned", a)))?;
    let idx = vm.current_regs()[a + 1]
        .clone()
        .ok_or_else(|| Error::internal(format!("register {} is not assigned", a + 1)))?;
    // Fast path: single-integer index into a pristine Array, preserving the
    // native semantics (negative indices, out-of-range reads return nil).
    if array_index_fast(vm, &recv)
        && let Value::Object(recv_obj) = &recv
        && let RValue::Array(arr) = &recv_obj.value
        && let Value::Integer(i) = &idx
    {
        let val = {
            let borrow = arr.borrow();
            let len = borrow.len() as i64;
            let mut i = *i;
            if i < 0 {
                i += len;
            }
            if i >= 0 && i < len {
                borrow[i as usize].clone()
            } else {
                Value::Nil
            }
        };
        vm.current_regs()[a].replace(val);
        return Ok(());
    }
    // Fast path: pristine Hash#[] with the Hash receiver does not need a full
    // method send; mruby inlines this case in OP_GETIDX too.
    if hash_index_fast(vm, &recv)
        && let Value::Object(o) = &recv
        && matches!(o.value, RValue::Hash(_))
    {
        let val = crate::yamrb::prelude::hash::mrb_hash_get_index(&recv, idx)?;
        vm.current_regs()[a].replace(val);
        return Ok(());
    }
    let val = mrb_funcall(vm, Some(recv), "[]", &[idx])?;
    vm.current_regs()[a].replace(val);
    Ok(())
}

pub(crate) fn op_getidx0(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let recv = vm.get_current_regs_cloned(b as usize)?;
    vm.current_regs()[a as usize].replace(Value::from_rc(recv));
    vm.current_regs()[a as usize + 1].replace(Value::Integer(0));
    do_op_send_with_id(vm, a as usize, None, a, RSym::new("[]".to_string()), 1)
}

pub(crate) fn op_matcherr(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let value = vm.get_current_regs_cloned(a)?;
    if value.is_truthy() {
        return Ok(());
    }
    Err(Error::TaggedError(
        "NoMatchingPatternError".to_string(),
        "pattern not matched".to_string(),
    ))
}

pub(crate) fn op_setidx(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let recv = vm.current_regs()[a]
        .clone()
        .ok_or_else(|| Error::internal(format!("register {} is not assigned", a)))?;
    let idx = vm.current_regs()[a + 1]
        .clone()
        .ok_or_else(|| Error::internal(format!("register {} is not assigned", a + 1)))?;
    let val = vm.current_regs()[a + 2]
        .clone()
        .ok_or_else(|| Error::internal(format!("register {} is not assigned", a + 2)))?;
    // Fast path only for in-bounds integer writes; anything else (appending,
    // out-of-range, non-integer) falls back to the native [] = which may
    // extend the array.
    if array_index_fast(vm, &recv)
        && let Value::Object(recv_obj) = &recv
        && let RValue::Array(arr) = &recv_obj.value
        && let Value::Integer(i) = &idx
    {
        let mut borrow = arr.borrow_mut();
        let len = borrow.len() as i64;
        let mut i = *i;
        if i < 0 {
            i += len;
        }
        if i >= 0 && i < len {
            borrow[i as usize] = val;
            return Ok(());
        }
        drop(borrow);
    }
    // Fast path: pristine Hash#[]= with a Hash receiver, same guard rationale
    // as the read side.
    if hash_aset_fast(vm, &recv)
        && let Value::Object(o) = &recv
        && matches!(o.value, RValue::Hash(_))
    {
        let _ = crate::yamrb::prelude::hash::mrb_hash_set_index(&recv, idx, val)?;
        return Ok(());
    }
    mrb_funcall(vm, Some(recv), "[]=", &[idx, val])?;
    Ok(())
}

pub(crate) fn op_jmp(vm: &mut VM, operand: &Fetched, end_pos: usize) -> Result<(), Error> {
    let a = operand.as_s()?;
    let offset = a as i16;
    let next_pc = calcurate_pc(
        &vm.current_irep,
        0,
        (end_pos as isize + offset as isize) as usize,
    );
    vm.pc.set(next_pc);
    Ok(())
}

pub(crate) fn op_jmpif(vm: &mut VM, operand: &Fetched, end_pos: usize) -> Result<(), Error> {
    let (a, b) = operand.as_bs()?;
    let val = vm.current_regs()[a as usize]
        .clone()
        .ok_or_else(|| Error::internal(format!("register {} is not assigned", a)))?;
    if val.is_truthy() {
        let offset = b as i16;
        let next_pc = calcurate_pc(
            &vm.current_irep,
            0,
            (end_pos as isize + offset as isize) as usize,
        );
        vm.pc.set(next_pc);
    }
    Ok(())
}

pub(crate) fn op_jmpnot(vm: &mut VM, operand: &Fetched, end_pos: usize) -> Result<(), Error> {
    let (a, b) = operand.as_bs()?;
    let val = vm.current_regs()[a as usize]
        .clone()
        .ok_or_else(|| Error::internal(format!("register {} is not assigned", a)))?;
    if val.is_falsy() {
        let offset = b as i16;
        let next_pc = calcurate_pc(
            &vm.current_irep,
            0,
            (end_pos as isize + offset as isize) as usize,
        );
        vm.pc.set(next_pc);
    }
    Ok(())
}

pub(crate) fn op_jmpnil(vm: &mut VM, operand: &Fetched, end_pos: usize) -> Result<(), Error> {
    let (a, b) = operand.as_bs()?;
    let val = vm.current_regs()[a as usize]
        .clone()
        .ok_or_else(|| Error::internal(format!("register {} is not assigned", a)))?;
    if val.is_nil() {
        let offset = b as i16;
        let next_pc = calcurate_pc(
            &vm.current_irep,
            0,
            (end_pos as isize + offset as isize) as usize,
        );
        vm.pc.set(next_pc);
    }
    Ok(())
}

pub(crate) fn op_jmpuw(vm: &mut VM, operand: &Fetched, end_pos: usize) -> Result<(), Error> {
    match vm.find_handler_pos(Some(CATCH_TYPE_ENSURE)) {
        None => op_jmp(vm, operand, end_pos),
        Some(target_pos) => {
            vm.pc.set(target_pos);

            consume_ensure_block(vm)?;
            op_jmp(vm, operand, end_pos)
        }
    }
}

fn consume_ensure_block(vm: &mut VM) -> Result<(), Error> {
    loop {
        let pc = vm.pc.get();
        if vm.current_irep.code.len() <= pc {
            // reached end of the IREP
            return Err(Error::internal(
                "end of opcode reached while consuming ensure block",
            ));
        }
        let op = *vm
            .current_irep
            .code
            .get(pc)
            .ok_or_else(|| Error::internal("end of opcode reached"))?;
        let operand = op.operand;
        vm.pc.set(pc + 1);

        if matches!(op.code, OpCode::RAISEIF) {
            return Ok(());
        }

        #[cfg(feature = "mrubyedge-debug")]
        if let Ok(v) = env::var("MRUBYEDGE_DEBUG") {
            let level: i32 = v.parse().unwrap_or(1);
            if level >= 2 {
                vm.debug_dump_to_stdout(32);
            }
            eprintln!(
                "{:?}: {:?} (pos={} len={})",
                op.code, operand, op.pos, op.len
            );
        }

        match consume_expr(vm, op.code, &operand, op.pos, op.len) {
            Ok(_) => {}
            Err(e @ (Error::Break(_) | Error::BlockReturn(_, _))) => {
                let exception = RException::from_error(vm, &e);
                vm.exception = Some(Rc::new(exception));
                continue;
            }
            Err(e) => {
                // snapshot at the deepest raise only (see
                // vm.rs twin site for rationale).
                if vm.exception.is_none() {
                    *vm.last_error_stack.borrow_mut() = vm.capture_error_stack();
                }
                return Err(e);
            }
        }
    }
}

pub(crate) fn op_except(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()?;
    let val = match vm.exception.take() {
        Some(e) => Value::from_rc(RObject::exception(e).to_refcount_assigned()),
        None => Value::Nil,
    };
    vm.current_regs()[a as usize].replace(val);
    Ok(())
}

pub(crate) fn op_rescue(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let val = vm.get_current_regs_cloned(a as usize)?;
    let exc_klass = vm.take_current_regs(b as usize)?;
    let RValue::Class(klass) = exc_klass.value.clone() else {
        return Err(Error::TaggedError(
            "TypeError".to_string(),
            "class or module required for rescue clause".to_string(),
        ));
    };
    // An ensure block reaches here on the way out of a body that did not raise,
    // and then R[a] holds the nil EXCEPT left behind.
    let is_rescued = match &val.value {
        RValue::Exception(exc) => {
            let etype = exc.error_type.borrow();
            etype.is_a(vm, klass)
        }
        _ => false,
    };
    vm.set_reg_value(b as usize, Value::Bool(is_rescued));
    Ok(())
}

pub(crate) fn op_raiseif(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()?;
    let val = vm.current_regs()[a as usize].as_ref().cloned();
    if let Some(Value::Object(o)) = val
        && let RValue::Exception(e) = &o.value
    {
        return Err(e.as_ref().error_type.borrow().clone());
    }
    Ok(())
}

#[inline]
pub(crate) fn op_move(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let regs = vm.current_regs();
    let val = match regs[b as usize].clone() {
        Some(v) => v,
        None => return Err(Error::internal(format!("register {} is not assigned", b))),
    };
    regs[a as usize].replace(val);
    Ok(())
}

pub(crate) fn op_ssend(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;
    do_op_send(vm, 0, None, a, b, c)
}

pub(crate) fn op_ssendb(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;
    let n: usize = (c & 0x0f) as usize;
    let k: usize = (c >> 4) as usize;
    do_op_send(vm, 0, Some(a as usize + n + k * 2 + 1), a, b, c)
}

pub(crate) fn op_send(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;

    // Attribute inline cache: when this site last resolved to an attr_accessor
    // closure for this receiver class and the method version has not moved,
    // the get/set runs directly on the receiver's IvarMap — no do_op_send, no
    // dispatch machinery. A redefinition bumps the version, and a receiver with
    // a singleton method resolves to a different class identity (same one the
    // fill used), so the entry goes cold and the normal send re-resolves.
    if (c & 0x0f) <= 1 && (c >> 4) == 0 {
        let site = vm.pc.get() - 1;
        let version = vm.method_version.get();
        let recv = vm.current_regs()[a as usize].clone();
        // Attr accessors are instance methods, so the receiver must be an
        // object; an immediate (or unassigned) receiver falls through.
        if let Some(Value::Object(recv_obj)) = &recv {
            // Snapshot the cache entry as Copy fields (+ the class pointer) so
            // the borrow ends before `singleton_or_this_class` needs `&mut vm`.
            let cached = {
                let cache = vm.current_irep.attr_cache.borrow();
                cache
                    .get(site)
                    .and_then(|slot| slot.as_ref())
                    .map(|e| (e.version, Rc::as_ptr(&e.klass) as usize, e.key, e.is_set))
            };
            if let Some((cached_version, cached_klass, key, is_set)) = cached
                && cached_version == version
                && cached_klass == Rc::as_ptr(&recv_obj.singleton_or_this_class(vm)) as usize
            {
                if is_set {
                    let value = vm.current_regs()[a as usize + 1].clone();
                    if let Some(value) = value {
                        recv_obj.set_ivar_by_id(key, value.clone());
                        vm.current_regs()[a as usize].replace(value);
                        vm.fast_native_hits.set(vm.fast_native_hits.get() + 1);
                        vm.attr_cache_hits.set(vm.attr_cache_hits.get() + 1);
                        return Ok(());
                    }
                } else {
                    let val = recv_obj.get_ivar_by_id(key);
                    vm.current_regs()[a as usize].replace(val);
                    vm.fast_native_hits.set(vm.fast_native_hits.get() + 1);
                    vm.attr_cache_hits.set(vm.attr_cache_hits.get() + 1);
                    return Ok(());
                }
            }
        }
    }

    do_op_send(vm, a as usize, None, a, b, c)
}

pub(crate) fn op_sendb(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;
    let n: usize = (c & 0x0f) as usize;
    let k: usize = (c >> 4) as usize;
    do_op_send(vm, a as usize, Some(a as usize + n + k * 2 + 1), a, b, c)
}

pub(crate) fn op_ssend0(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    do_op_send(vm, 0, None, a, b, 0)
}

pub(crate) fn op_send0(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    do_op_send(vm, a as usize, None, a, b, 0)
}

pub(crate) fn op_blkcall(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let block = vm.get_current_regs_cloned(a as usize)?;
    if !matches!(block.value, RValue::Proc(_)) {
        return Err(Error::TaggedError(
            "TypeError".to_string(),
            "wrong type (expected Proc)".to_string(),
        ));
    }
    // codegen emits BLKCALL only for nk == 0 && n < 15, so b fits the argument nibble.
    do_op_send_with_id(vm, a as usize, None, a, RSym::new("call".to_string()), b)
}

/// Tries to execute a tagged send inline, skipping the native-call machinery
/// (arg Vec, fn-table lookup, frame bookkeeping). Attribute accessors are
/// handled first as a direct IvarMap read/write; then numeric ops (modulo,
/// power, spaceship, `!=`) run when their operands are numeric. Any other
/// operand type falls through to the real method so coercion and errors keep
/// their native behavior. Returns `None` when not eligible.
///
/// The tag is bound to the method's `func` identity at registration and read
/// back through a version-and-class-guarded dispatch cache hit, so a
/// redefinition (new func, bumped version) can never fast-path with a stale
/// tag. Only value-only operations are tagged — the Comparable family is not,
/// because it dispatches `<=>` dynamically and would bypass user overrides.
fn try_fast_op(
    vm: &mut VM,
    method: &RProc,
    recv: &Value,
    a: usize,
    n: usize,
) -> Option<Result<Value, Error>> {
    let op = method.fast_op?;

    // attr_accessor closures: a direct IvarMap access on the receiver, no call
    // frame. The receiver can be any object; the guard is the dispatch cache's
    // receiver-class check, and redefinition replaces the method + tag.
    if let FastOp::AttrGet | FastOp::AttrSet = op {
        let key = method.attr_key?;
        let recv_rc = match recv {
            Value::Object(o) => o.clone(),
            _ => return None,
        };
        let result = match op {
            FastOp::AttrGet => Ok(recv_rc.get_ivar_by_id(key)),
            _ => {
                let value = vm.current_regs()[a + 1].clone()?;
                recv_rc.set_ivar_by_id(key, value.clone());
                Ok(value)
            }
        };
        vm.fast_native_hits.set(vm.fast_native_hits.get() + 1);
        return Some(result);
    }

    // First operand: the receiver. Every tagged numeric op expects one.
    let (recv_i, recv_f) = match recv {
        Value::Integer(i) => (Some(*i), None),
        Value::Float(f) => (None, Some(*f)),
        _ => return None,
    };
    // Second operand (binary ops only). A missing or non-numeric operand falls
    // through to the native method, which reports arity/type errors as usual.
    let arg = if n == 0 {
        None
    } else {
        vm.current_regs()[a + 1].clone()
    };
    let (arg_i, arg_f) = match &arg {
        Some(Value::Integer(i)) => (Some(*i), None),
        Some(Value::Float(f)) => (None, Some(*f)),
        _ => (None, None),
    };

    // Numeric ordering used by `<=>`. Integer pairs compare exactly as i64
    // (matching Object#<=> and the comparison opcodes); mixed pairs promote to
    // f64, exactly like the native closure.
    let order = |a: f64, b: f64| -> i64 {
        if a < b {
            -1
        } else if a > b {
            1
        } else {
            0
        }
    };
    let spaceship = || -> Option<i64> {
        match (recv_i, recv_f, arg_i, arg_f) {
            (Some(a), None, Some(b), None) => Some(if a < b {
                -1
            } else if a > b {
                1
            } else {
                0
            }),
            (None, Some(a), None, Some(b)) => Some(order(a, b)),
            (Some(a), None, None, Some(b)) => Some(order(a as f64, b)),
            (None, Some(a), Some(b), None) => Some(order(a, b as f64)),
            _ => None,
        }
    };

    // Each arm produces a `Result`; fall-throughs (`return None`) skip the
    // counter below because the native method runs instead.
    let result: Result<Value, Error> = match op {
        FastOp::IntModTrunc => {
            // Prelude semantics: truncated modulo over two Integers. A zero
            // divisor falls through to the native method, which panics on `%0`
            // exactly as the prelude's own implementation would.
            let (Some(a), Some(b)) = (recv_i, arg_i) else {
                return None;
            };
            if b == 0 {
                return None;
            }
            Ok(Value::Integer(a % b))
        }
        FastOp::IntModFloored => {
            // Engine compat semantics: floored modulo; Integer divisor keeps
            // the result Integer, Float divisor promotes to Float.
            if let (Some(a), Some(b)) = (recv_i, arg_i) {
                if b == 0 {
                    Err(Error::ZeroDivisionError)
                } else {
                    Ok(Value::Integer(rubylike_mod_i64(a, b)))
                }
            } else {
                let a = recv_i.map(|i| i as f64).or(recv_f)?;
                let b = arg_i.map(|i| i as f64).or(arg_f)?;
                if b == 0.0 {
                    Err(Error::ZeroDivisionError)
                } else {
                    Ok(Value::Float(rubylike_mod_f64(a, b)))
                }
            }
        }
        FastOp::FloatModFloored => {
            let a = recv_f?;
            let b = arg_f.or_else(|| arg_i.map(|i| i as f64))?;
            if b == 0.0 {
                Err(Error::ZeroDivisionError)
            } else {
                Ok(Value::Float(rubylike_mod_f64(a, b)))
            }
        }
        FastOp::IntPow => {
            let base = recv_i?;
            match (arg_i, arg_f) {
                // Same `pow` as the native: overflow panics in debug and wraps
                // in release, so both paths stay identical.
                (Some(exp), _) if exp >= 0 => Ok(Value::Integer(base.pow(exp as u32))),
                (Some(exp), _) => Ok(Value::Float((base as f64).powf(exp as f64))),
                (None, Some(exp)) => Ok(Value::Float((base as f64).powf(exp))),
                _ => return None,
            }
        }
        FastOp::FloatPow => {
            let base = recv_f?;
            let exp = arg_f.or_else(|| arg_i.map(|i| i as f64))?;
            Ok(Value::Float(base.powf(exp)))
        }
        FastOp::NumSpaceship => Ok(Value::Integer(spaceship()?)),
        // Object#!= uses ValueEquality, where Integer and Float are never
        // equal; only same-kind operands are fast-pathed, mixed falls back.
        FastOp::NumNe => match (recv_i, recv_f, arg_i, arg_f) {
            (Some(a), None, Some(b), None) => Ok(Value::Bool(a != b)),
            (None, Some(a), None, Some(b)) => Ok(Value::Bool(a != b)),
            _ => return None,
        },
        // Handled above; unreachable here.
        FastOp::AttrGet | FastOp::AttrSet => return None,
    };
    // Count every inline handling (result or raised error) — fall-throughs
    // returned None above and did not reach here — so tests can prove the fast
    // path runs and stays disabled for redefined (untagged) methods.
    vm.fast_native_hits.set(vm.fast_native_hits.get() + 1);
    Some(result)
}

pub(crate) fn do_op_send(
    vm: &mut VM,
    recv_index: usize,
    blk_index: Option<usize>,
    a: u8,
    b: u8,
    c: u8,
) -> Result<(), Error> {
    let method_id = vm.current_irep.syms[b as usize].clone();
    do_op_send_with_id(vm, recv_index, blk_index, a, method_id, c)
}

pub(crate) fn do_op_send_with_id(
    vm: &mut VM,
    recv_index: usize,
    blk_index: Option<usize>,
    a: u8,
    method_id: RSym,
    c: u8,
) -> Result<(), Error> {
    let mut n: usize = (c & 0x0f) as usize;
    let k: usize = (c >> 4) as usize;
    let irep = vm.current_irep.clone();

    if method_id.name == "__debug__vm_info" {
        // Special debug method to dump VM info
        vm.debug_dump_to_stdout(32);
        vm.current_regs()[a as usize].replace(Value::Nil);
        return Ok(());
    }

    let block_index = a as usize + n + k * 2 + 1;

    // Receiver as an unboxed register value; it is boxed to `recv` only when
    // the non-fast path needs an RObject (class identity for dispatch, the
    // native call and the error breadcrumb).
    let recv_value = if recv_index == 0 {
        vm.current_regs()[0]
            .clone()
            .ok_or_else(|| Error::internal("register 0 is not assigned"))?
    } else {
        vm.current_regs()[recv_index]
            .clone()
            .ok_or_else(|| Error::internal(format!("register {} is not assigned", recv_index)))?
    };

    if k > 0 {
        let mut map = RHashMap::default();
        for i in 0..k {
            let key = vm
                .get_current_regs_cloned(a as usize + n + i * 2 + 1)?
                .intern()?;
            let val = vm.get_reg_value(a as usize + n + i * 2 + 2);
            map.insert(key, val);
        }
        vm.kargs.borrow_mut().replace(map);
    } else if vm.kargs.borrow().as_ref().is_some_and(|m| !m.is_empty()) {
        // Reset the slot so a k>0 send that failed before its callee ran
        // cannot leak a stale kwargs map into the next call; an empty map
        // keeps op_enter's KArgs upper-chain intact. The common no-kwargs
        // path is already empty, so only clear after a real map.
        vm.kargs.borrow_mut().replace(RHashMap::default());
    }

    // The block value is captured here but only appended to the argument
    // vector for native calls; Ruby callees read it from the registers.
    let mut block_val: Option<Value> = None;
    if let Some(blk_index) = blk_index {
        let blk_val = vm.current_regs()[blk_index]
            .clone()
            .ok_or_else(|| Error::internal(format!("register {} is not assigned", blk_index)))?;
        if matches!(blk_val, Value::Symbol(_)) {
            let proc_val = mrb_funcall(vm, Some(blk_val), "to_proc", &[])?;
            block_val = Some(proc_val);
        } else {
            block_val = Some(blk_val);
        }
    } else {
        // When no block is provided, do not push a nil placeholder
        vm.current_regs()[block_index].replace(Value::Nil);
    }

    let klass = recv_value.get_class(vm);
    let klass = if klass.is_singleton {
        klass
    } else {
        recv_value.singleton_or_this_class(vm)
    };
    let mut via_method_missing = false;
    // Inline dispatch cache: the send site is the current pc (the loop
    // advanced past this instruction). A hit needs both the version stamp
    // and the receiver class identity; any redefinition bumps the version,
    // so a stale entry can never hit.
    let site = vm.pc.get() - 1;
    let version = vm.method_version.get();
    let was_cache_hit = vm
        .current_irep
        .send_cache
        .borrow()
        .get(site)
        .is_some_and(|slot| slot.is_some());
    let cached = {
        let caches = vm.current_irep.send_cache.borrow();
        caches
            .get(site)
            .and_then(|slot| slot.as_ref())
            .filter(|entry| entry.version == version && Rc::ptr_eq(&entry.klass, &klass))
            .map(|entry| (entry.owner.clone(), entry.method.clone()))
    };
    let (owner_module, method) = match cached {
        Some(hit) => hit,
        None => {
            let resolved = resolve_method_by_id(&klass, method_id.id)
                .or_else(|| {
                    unshift_method_name(vm, &method_id, a as usize, n + k * 2 + 1);
                    n += 1;
                    via_method_missing = true;
                    resolve_method_by_id(&klass, intern_symbol("method_missing"))
                })
                .ok_or_else(|| {
                    Error::Internal(format!(
                        "[BUG] method_missing not defined. {} for {}",
                        method_id.name,
                        klass.full_name()
                    ))
                })?;
            // Only direct hits are cached; method_missing stays uncached.
            if !via_method_missing {
                let mut caches = vm.current_irep.send_cache.borrow_mut();
                if let Some(slot) = caches.get_mut(site) {
                    *slot = Some(SendCacheEntry {
                        version,
                        klass: klass.clone(),
                        owner: resolved.0.clone(),
                        method: resolved.1.clone(),
                    });
                }
                // Mirror into the attr cache: when the resolved method is an
                // attr_accessor closure, record the ivar so op_send can run the
                // access without entering do_op_send at all.
                let tag = resolved.1.fast_op;
                if let Some(FastOp::AttrGet | FastOp::AttrSet) = tag
                    && let Some(key) = resolved.1.attr_key
                {
                    let is_set = matches!(tag, Some(FastOp::AttrSet));
                    let mut attrs = vm.current_irep.attr_cache.borrow_mut();
                    if let Some(slot) = attrs.get_mut(site) {
                        *slot = Some(AttrCacheEntry {
                            version,
                            klass: klass.clone(),
                            key,
                            is_set,
                        });
                    }
                }
            }
            resolved
        }
    };

    // inline numeric/attr fast path in do_op_send (the op_send
    // attr inline cache handles the common attribute case before this). Runs
    // when the send site's cache slot is populated with no kwargs or block;
    // the resolved `method` comes from the version-and-class-guarded entry (or
    // a fresh resolve when the slot is stale), so the tag always matches the
    // method that would run. Errors mirror the native call: the result
    // register is cleared before the exception propagates to the interpreter.
    if was_cache_hit && k == 0 && blk_index.is_none() {
        match try_fast_op(vm, &method, &recv_value, a as usize, n) {
            Some(Ok(val)) => {
                vm.current_regs()[a as usize].replace(val);
                return Ok(());
            }
            Some(Err(e)) => {
                vm.current_regs()[a as usize].replace(Value::Nil);
                return Err(e);
            }
            None => {}
        }
    }

    // guard the callee's register window before pushing its
    // frame; unbounded recursion must raise SystemStackError, not panic.
    if let Some(irep) = method.irep.as_ref() {
        vm.check_frame_window(a as usize, irep.nregs)?;
    }
    // lazy frame label built from the unboxed receiver; the
    // receiver class is cloned (no allocation) and the name is resolved from
    // the frame irep at error time.
    let receiver = match &recv_value {
        Value::Object(o) => match &o.value {
            RValue::Class(c) => CallerReceiver::Class(c.clone()),
            RValue::Module(m) => CallerReceiver::Module(m.clone()),
            _ => CallerReceiver::Instance(o.get_class(vm)),
        },
        _ => CallerReceiver::Instance(recv_value.get_class(vm)),
    };
    vm.push_breadcrumb(
        "do_op_send",
        Some(CallerLabel::Send {
            receiver,
            method_id: method_id.id,
            use_method_missing: via_method_missing && method.is_rb_func,
        }),
        Some(a as usize),
        Some(irep.clone()),
        Some(vm.pc.get().saturating_sub(1)),
    );

    // The receiver is already at reg[a] for op_send (recv_index == a), so the
    // common path needs no write. Super sends (op_ssend: receiver lives in
    // reg 0, result in reg[a]) place it at reg[a] so the callee reads it as
    // self after the register-window shift.
    if recv_index != a as usize {
        vm.set_reg(a as usize, recv_value.to_rc());
    }

    if !method.is_rb_func {
        // Build the argument slice only for native calls (Ruby callees read
        // the registers directly). After method_missing the name sits at
        // a+1 and the original args were shifted up by one. Both this build
        // and get_fn run before the KArgs frame, so an error path cannot
        // leave an unpopped frame. The register window is copied into a stack
        // buffer as unboxed `Option<Value>`; arity is at most 15 + method
        // name + block, so the buffer always fits.
        let mm = via_method_missing;
        let native_n = n - mm as usize;
        let first_arg = a as usize + 1 + mm as usize;
        let mut buf = arg_buf();
        let mut len = 0usize;
        if mm {
            buf[len] = Some(vm.current_regs()[a as usize + 1].clone().ok_or_else(|| {
                Error::internal(format!("register {} is not assigned", a as usize + 1))
            })?);
            len += 1;
        }
        for i in 0..native_n {
            buf[len] = Some(vm.current_regs()[first_arg + i].clone().ok_or_else(|| {
                Error::internal(format!("register {} is not assigned", first_arg + i))
            })?);
            len += 1;
        }
        if let Some(blk) = block_val {
            buf[len] = Some(blk);
            len += 1;
        }
        let args = &buf[..len];

        let func = vm
            .get_fn(method.func.unwrap())
            .ok_or_else(|| Error::internal("function not found"))?;

        // no keyword arguments means no KArgs frame. Note
        // that a native method inside a Ruby method with live kwargs then
        // reads the outer KArgs via get_kwargs() instead of an empty one;
        // no engine native method inspects kwargs today.
        if k > 0 {
            kwarg_op_enter(vm, 0);
        }
        vm.current_regs_offset += a as usize;

        let res = func(vm, args);

        if k > 0 {
            kwarg_op_return(vm);
        }

        vm.current_regs_offset -= a as usize;
        for i in (a as usize + 1)..block_index {
            vm.current_regs()[i].take();
        }

        match res {
            Ok(val) => {
                vm.current_regs()[a as usize].replace(val);
                vm.pop_breadcrumb();
            }
            Err(e) => {
                vm.current_regs()[a as usize].replace(Value::Nil);
                // capture the backtrace at this send before
                // popping its breadcrumb (deepest frame wins), mark the
                // exception as pending so outer conversions skip their own
                // snapshots, then pop our breadcrumb so failed native calls
                // do not leak frames into later backtraces.
                if vm.exception.is_none() {
                    *vm.last_error_stack.borrow_mut() = vm.capture_error_stack();
                    let exception = RException::from_error(vm, &e);
                    vm.exception = Some(Rc::new(exception));
                }
                vm.pop_breadcrumb();
                return Err(e);
            }
        }

        return Ok(());
    }

    push_callinfo(vm, method_id.id, n, Some(owner_module), a as usize, false);

    // Set has_block flag based on whether a block was provided
    if let Some(ci) = vm.callinfo_stack.last() {
        ci.has_block.set(blk_index.is_some());
    }

    vm.pc.set(0);
    vm.current_irep = method.irep.ok_or_else(|| Error::internal("empry irep"))?;
    vm.current_regs_offset += a as usize;
    Ok(())
}

fn unshift_method_name(vm: &mut VM, method_id: &RSym, a: usize, total_args: usize) {
    let method_name = RObject::symbol_rc(method_id);
    for i in (a + 1..=a + total_args).rev() {
        let val = vm.current_regs().get(i).and_then(|r| r.as_ref().cloned());
        if let Some(v) = val.as_ref() {
            let _ = mrb_call_inspect(vm, v);
        }
        vm.set_reg_value(i + 1, val.unwrap_or(Value::Nil));
    }
    vm.set_reg(a + 1, method_name);
}

fn kwarg_op_enter(vm: &mut VM, rest_pos: usize) {
    let kwrest_reg = Cell::new(rest_pos);
    let current_arg = if let Some(args) = vm.kargs.borrow_mut().take() {
        let upper = vm.current_kargs.borrow_mut().take();
        KArgs {
            args: RefCell::new(args),
            kwrest_reg,
            upper,
        }
    } else {
        KArgs {
            args: RefCell::new(RHashMap::default()),
            kwrest_reg,
            upper: None,
        }
    };
    vm.current_kargs.borrow_mut().replace(Rc::new(current_arg));
}

fn kwarg_op_return(vm: &mut VM) {
    let old_kargs = vm.current_kargs.borrow_mut().take();
    if let Some(upper) = old_kargs.as_ref().and_then(|kargs| kargs.upper.clone()) {
        vm.current_kargs.borrow_mut().replace(upper);
    }
}

pub(crate) fn op_call(vm: &mut VM, _operand: &Fetched) -> Result<(), Error> {
    vm.push_breadcrumb(
        "op_call",
        Some(CallerLabel::Static("<tailcall>")),
        None,
        Some(vm.current_irep.clone()),
        Some(vm.pc.get().saturating_sub(1)),
    );
    push_callinfo(vm, intern_symbol("<tailcall>"), 0, None, 0, false);

    vm.pc.set(0);
    let proc = vm.current_regs()[0]
        .clone()
        .ok_or_else(|| Error::internal("proc not found"))?
        .to_rc();
    match &proc.value {
        RValue::Proc(proc) => {
            vm.current_irep = proc
                .irep
                .as_ref()
                .ok_or_else(|| Error::internal("empry irep"))?
                .clone();
        }
        _ => unreachable!("call must be called on proc"),
    }
    Ok(())
}

/// OP_SUPER with this arg count forwards the arguments packed in an array by
/// ARGARY (regs[a+1]) instead of reading them from consecutive registers.
const CALL_MAXARGS: usize = 15;

pub(crate) fn op_super(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let splat = b as usize == CALL_MAXARGS;
    let (sym_id, super_method_id, owner_module) = {
        let callinfo = vm
            .callinfo_stack
            .last()
            .ok_or_else(|| Error::internal("no current callinfo"))?;
        (
            symbol_name(callinfo.method_id),
            callinfo.method_id,
            callinfo
                .method_owner
                .clone()
                .ok_or_else(|| Error::RuntimeError("super called outside of method".to_string()))?,
        )
    };
    let recv = vm.getself()?;

    let mut buf = arg_buf();
    let mut tmp: Vec<Option<Value>> = Vec::new();
    let args: &[Option<Value>];
    let arg_count: usize;
    if splat {
        let ary = vm.get_current_regs_cloned((a + 1) as usize)?;
        match &ary.value {
            RValue::Array(inner) => {
                let inner = inner.borrow();
                arg_count = inner.len();
                if arg_count <= buf.len() {
                    for (i, v) in inner.iter().enumerate() {
                        buf[i] = Some(v.clone());
                    }
                    args = &buf[..arg_count];
                } else {
                    tmp = inner.iter().map(|v| Some(v.clone())).collect();
                    args = tmp.as_slice();
                }
            }
            _ => {
                buf[0] = Some(Value::from_rc(ary));
                arg_count = 1;
                args = &buf[..1];
            }
        }
    } else {
        arg_count = b as usize;
        args = reg_args(vm, (a + 1) as usize, arg_count, &mut buf, &mut tmp)?;
    }

    let klass = match recv.rvalue() {
        Some(RValue::Instance(ins)) => ins.class.clone(),
        _ => recv.initialize_or_get_singleton_class(vm),
    };
    let (next_owner, method) =
        resolve_next_method(&klass, &sym_id, &owner_module).ok_or_else(|| {
            Error::NoMethodError(format!("{} for {}", sym_id.clone(), klass.full_name()))
        })?;
    if !method.is_rb_func {
        let func = vm.get_fn(method.func.unwrap()).ok_or_else(|| {
            Error::internal(format!("functon registerd but no entry found: {}", sym_id))
        })?;
        let res = func(vm, args);
        for i in (a as usize + 1)..(a as usize + arg_count + 1) {
            vm.current_regs()[i].take();
        }
        match res {
            Ok(val) => {
                vm.current_regs()[a as usize].replace(val);
            }
            Err(e) => {
                vm.current_regs()[a as usize].replace(Value::Nil);
                return Err(e);
            }
        }
        return Ok(());
    }

    // guard the callee's register window before pushing its
    // frame; unbounded recursion must raise SystemStackError, not panic.
    if let Some(irep) = method.irep.as_ref() {
        vm.check_frame_window(a as usize, irep.nregs)?;
    }

    // A splat super keeps its block at a+2; capture it before the arg writes
    // overwrite that slot, then place it at the callee's block local.
    let blk = if splat {
        vm.current_regs()[a as usize + 2].clone()
    } else {
        None
    };
    for (i, arg) in args.iter().enumerate() {
        vm.current_regs()[a as usize + 1 + i] = arg.clone();
    }
    if splat {
        vm.set_reg(
            a as usize + arg_count + 1,
            blk.map(|v| v.to_rc()).unwrap_or_else(RObject::nil_rc),
        );
    }

    vm.push_breadcrumb(
        "super",
        Some(CallerLabel::Super {
            method_id: super_method_id,
        }),
        None,
        Some(vm.current_irep.clone()),
        Some(vm.pc.get().saturating_sub(1)),
    );

    vm.set_reg_value(a as usize, recv.clone());
    push_callinfo(
        vm,
        method.sym_id.unwrap(),
        b as usize,
        Some(next_owner),
        a as usize,
        false,
    );

    vm.pc.set(0);
    vm.current_irep = method
        .irep
        .as_ref()
        .ok_or_else(|| Error::internal("empty irep"))?
        .clone();
    vm.current_regs_offset += a as usize;
    Ok(())
}

/// OP_ARGARY: packs the running method's arguments (leading args, optional
/// rest array and post args) into a new array at regs[a] and moves the block
/// next to it, so a following bare `super` can forward them. The operand is
/// (m5:r1:m5:d1:lv4); args are read from regs[1..] of the current frame.
pub(crate) fn op_argary(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, s) = operand.as_bs()?;
    let m1 = ((s >> 11) & 0x1f) as usize;
    let r = ((s >> 10) & 0x1) as usize;
    let m2 = ((s >> 5) & 0x1f) as usize;
    let lv = (s & 0xf) as usize;
    if lv != 0 {
        return Err(Error::internal(
            "super from a nested block (ARGARY lv>0) is not supported",
        ));
    }

    let mut values: Vec<Value> = Vec::new();
    let mut i = 0;
    for _ in 0..m1 {
        values.push(Value::from_rc(vm.get_current_regs_cloned(i + 1)?));
        i += 1;
    }
    if r == 1 {
        let rest = vm.get_current_regs_cloned(i + 1)?;
        if let RValue::Array(ary) = &rest.value {
            for item in ary.borrow().iter() {
                values.push(item.clone());
            }
        } else {
            values.push(Value::from_rc(rest));
        }
        i += 1;
    }
    for _ in 0..m2 {
        values.push(Value::from_rc(vm.get_current_regs_cloned(i + 1)?));
        i += 1;
    }
    let array = RObject::array(values);
    vm.set_reg(a as usize, array.to_refcount_assigned());
    // The block sits right after the args; move it next to the array.
    let blk = vm.get_current_regs_cloned(i + 1)?;
    vm.set_reg((a + 1) as usize, blk);
    Ok(())
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
pub(crate) struct EnterArgInfo {
    pub n1: u32,
    pub m1: u32,
    pub o: u32,
    pub r: u32,
    pub m2: u32,
    pub k: u32,
    pub d: u32,
    pub b: u32,
}

impl From<u32> for EnterArgInfo {
    fn from(val: u32) -> Self {
        EnterArgInfo {
            n1: (val & ENTER_N1_MASK) >> 23,
            m1: (val & ENTER_M1_MASK) >> 18,
            o: (val & ENTER_O_MASK) >> 13,
            r: (val & ENTER_R_MASK) >> 12,
            m2: (val & ENTER_M2_MASK) >> 7,
            k: (val & ENTER_K_MASK) >> 2,
            d: (val & ENTER_D_MASK) >> 1,
            b: (val & ENTER_B_MASK),
        }
    }
}

pub(crate) fn op_enter(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_w()?;
    // n_args lives on the VM because call_block hides the
    // current callinfo while the callee runs, which made every optional
    // argument fall back to its default for funcall-invoked methods.
    let argc = vm.current_n_args.get();
    let arg_info = EnterArgInfo::from(a);
    // proc.h MRB_ASPEC_NOBLOCK: n1 (bit 23) refuses a block argument.
    let has_block = vm.current_callinfo().is_some_and(|ci| ci.has_block.get());
    if arg_info.n1 == 1 && has_block {
        return Err(Error::ArgumentError("no block accepted".to_string()));
    }
    // The block arrives at the call-based slot (regs[argc+1]) and must land
    // on the signature-based block local (regs[len+1]); capture it before the
    // rest packing overwrites the call-based slot, and place it at the end.
    let block_len = arg_info.m1 + arg_info.o + arg_info.r + arg_info.m2;
    let blk = vm.current_regs()[argc + 1].clone();
    let m1_argc = arg_info.m1 as usize;
    for i in 0..m1_argc {
        match vm.current_regs()[i + 1].as_ref() {
            Some(_) => {}
            None => {
                return Err(Error::ArgumentError(format!(
                    "argument {} not passed",
                    i + 1
                )));
            }
        }
    }
    let optional_arg = arg_info.o as usize;
    if optional_arg > 0 {
        let m2_argc = arg_info.m2 as usize;
        let total_preset_args = argc.saturating_sub(m1_argc + m2_argc);
        for peek_pc in 0..total_preset_args {
            match vm.current_irep.code[vm.pc.get() + peek_pc].code {
                OpCode::JMP => {}
                _ => {
                    unreachable!("unexpected opcode while processing optional args")
                }
            }
        }
        vm.pc.set(vm.pc.get() + total_preset_args);
    }

    let splat_arg = arg_info.r as usize;
    if splat_arg == 1 {
        let total_args = argc;
        let passed_args = total_args.saturating_sub(m1_argc);
        let mut array = Vec::new();
        for i in 0..passed_args {
            if let Some(val) = vm.current_regs()[m1_argc + i + 1].take() {
                array.push(val);
            }
        }
        let splat = RObject::array(array);
        vm.set_reg(m1_argc + splat_arg, splat.to_refcount_assigned());
    }
    let kwrest_arg = arg_info.d as usize;
    let kwrest_pos = if kwrest_arg == 1 {
        m1_argc + splat_arg + kwrest_arg
    } else {
        0
    };
    // only push a KArgs frame when the callee actually
    // accepts keyword arguments; methods without them never read
    // current_kargs, so the frame would be pure overhead.
    // Note: when the callinfo is hidden (mrb_funcall/call_block take it),
    // kargs_pushed cannot be recorded and the None-ci op_return never pops
    // such a frame. Pre-existing and dormant (no engine call passes kwargs
    // through mrb_funcall); revisit with the callinfo machinery.
    if arg_info.k > 0 || kwrest_arg == 1 {
        kwarg_op_enter(vm, kwrest_pos);
        if let Some(ci) = vm.callinfo_stack.last() {
            ci.kargs_pushed.set(true);
        }
    }
    if kwrest_arg == 1 {
        let mut map = RHashMap::default();
        for (k, v) in vm
            .get_kwargs()
            .ok_or_else(|| Error::RuntimeError("kwargs not defined".to_string()))?
            .iter()
        {
            let k = Value::Symbol(RSym::new(k.clone()).id);
            map.insert(k.as_hash_key()?, (k, v.clone()));
        }

        let kwrest = RObject::hash(map);
        vm.set_reg(kwrest_pos, kwrest.to_refcount_assigned());
    }
    // Land the block on the signature-based block local, unless that slot
    // already holds an argument (native funcalls can pass a proc as a
    // positional argument to a `&block` parameter, e.g. Enumerable#map).
    if vm.current_regs()[(block_len + 1) as usize].is_none() {
        vm.set_reg(
            (block_len + 1) as usize,
            blk.map(|v| v.to_rc()).unwrap_or_else(RObject::nil_rc),
        );
    }

    Ok(())
}

pub(crate) fn op_key_p(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let key = &vm.current_irep.syms[b as usize];
    let key_robj = RObject::symbol_rc(key);

    let (val, kwrest_pos) = {
        let kargs = vm.current_kargs.borrow();
        let kargs = kargs
            .as_ref()
            .ok_or_else(|| Error::internal("no kargs found"))?;

        let kwrest_pos = kargs.kwrest_reg.get();

        (
            RObject::boolean_rc(kargs.args.borrow().contains_key(key)),
            kwrest_pos,
        )
    };

    if kwrest_pos != 0 {
        let kwrest = vm.get_current_regs_cloned(kwrest_pos)?;
        mrb_hash_delete(&Value::from_rc(kwrest), Value::from_rc(key_robj))?;
    }

    vm.set_reg(a as usize, val);
    Ok(())
}

pub(crate) fn op_keyend(vm: &mut VM, _operand: &Fetched) -> Result<(), Error> {
    match vm.current_kargs.borrow().as_deref() {
        Some(kargs) => {
            let is_empty = kargs.args.borrow().is_empty();
            if is_empty {
                Ok(())
            } else {
                Err(Error::ArgumentError(
                    "unexpected keyword arguments".to_string(),
                ))
            }
        }
        None => Err(Error::internal("no kargs found")),
    }
}

pub(crate) fn op_karg(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let val = {
        let key = vm.current_irep.syms[b as usize].clone();
        let kargs = vm.current_kargs.borrow();
        let kargs = kargs
            .as_ref()
            .ok_or_else(|| Error::internal("no kargs found"))?;

        let mut args = kargs.args.borrow_mut();
        args.remove(&key).ok_or_else(|| {
            Error::ArgumentError(format!("keyword argument '{}' not found", key.name))
        })?
    };
    vm.set_reg_value(a as usize, val);
    Ok(())
}

pub(crate) fn op_return(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let value = vm.current_regs()[a].clone();
    do_return(vm, value)
}

pub(crate) fn op_retself(vm: &mut VM, _operand: &Fetched) -> Result<(), Error> {
    let value = vm.current_regs()[0].clone();
    do_return(vm, value)
}

pub(crate) fn op_retnil(vm: &mut VM, _operand: &Fetched) -> Result<(), Error> {
    do_return(vm, Some(Value::Nil))
}

pub(crate) fn op_rettrue(vm: &mut VM, _operand: &Fetched) -> Result<(), Error> {
    do_return(vm, Some(Value::Bool(true)))
}

pub(crate) fn op_retfalse(vm: &mut VM, _operand: &Fetched) -> Result<(), Error> {
    do_return(vm, Some(Value::Bool(false)))
}

fn do_return(vm: &mut VM, value: Option<Value>) -> Result<(), Error> {
    let old_irep = vm.current_irep.clone();
    let nregs = old_irep.nregs;
    // let no_return = vm.current_callinfo.is_some();

    // Capture the caller's register window for closure locals. The window
    // includes the block proc stored by op_block/op_lambda whose environ is
    // THIS env; keeping that reference inside captured forms an immortal
    // Rc cycle (env->proc->env). Null out such self-references so the env
    // is kept alive by the proc alone, not by a cycle.
    if let Some(environ) = vm.cur_env.get(&vm.current_irep.__id).cloned() {
        if environ.__irep_id == vm.current_irep.__id {
            let regs0_cloned: Vec<Option<Rc<RObject>>> = vm.current_regs()[0..nregs]
                .iter()
                .map(|r| r.as_ref().map(|v| v.to_rc()))
                .collect();
            environ.capture_no_clone(regs0_cloned);
            if let Some(ref mut captured) = *environ.captured.borrow_mut() {
                for slot in captured.iter_mut() {
                    if let Some(obj) = slot
                        && let RValue::Proc(p) = &obj.value
                        && let Some(pe) = &p.environ
                        && Rc::ptr_eq(&environ, pe)
                    {
                        *slot = None;
                    }
                }
            }
        }
        environ.as_ref().expire();
        vm.has_env_ref.remove(&vm.current_irep.__id);
    }

    let regs0 = vm.current_regs();
    if let Some(value) = value {
        regs0[0].replace(value);
    }
    // TODO: inspect if this is needed
    // if nregs > 0 && no_return {
    //     regs0[1..=nregs].iter_mut().for_each(|reg| {
    //         reg.take();
    //     });
    // }

    let ci = vm.callinfo_stack.pop();
    if ci.is_none() || ci.as_ref().is_some_and(|c| c.is_funcall) {
        vm.pop_breadcrumb();
        // When called from mrb_funcall, return error if there's an exception

        if let Some(e) = &vm.exception {
            return Err(e.error_type.borrow().clone());
        }
        // For normal completion, set preemption flag and terminate
        vm.flag_preemption.set(true);
        return Ok(());
    }

    let ci = ci.unwrap();
    vm.current_irep = ci.pc_irep.clone();
    vm.pc.set(ci.pc);
    vm.current_regs_offset = ci.current_regs_offset;
    vm.target_class = ci.target_class.clone();
    if vm.current_regs()[0].is_none() {
        unreachable!("debug");
    }

    if ci.kargs_pushed.get() {
        kwarg_op_return(vm);
    }

    vm.pop_breadcrumb();
    Ok(())
}

pub(crate) fn op_return_blk(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let val = vm.get_reg_value(a);

    // a plain method frame (return inside a while/until body)
    // carries no block environment; OP_RETURN_BLK is then just a local return.
    // Blocks/lambdas run with an is_funcall callinfo (call_block), methods
    // entered through a send do not.
    let is_funcall = vm.callinfo_stack.last().is_some_and(|c| c.is_funcall);
    if !is_funcall {
        return op_return(vm, operand);
    }

    // Block/lambda frame: unwind to the nearest enclosing lambda (its return
    // is local to the lambda), else to the defining method.
    let mut env = vm.upper.clone();
    while let Some(e) = env.clone() {
        if e.is_lambda.get() {
            return Err(Error::BlockReturn(e.closure_irep_id, val));
        }
        env = e.upper.clone();
    }
    let outer = vm
        .get_outermost_env()
        .ok_or_else(|| Error::internal("block return without environment"))?;
    if vm.root_irep_id.get() == Some(outer.__irep_id) {
        return Err(Error::LocalJumpError("unexpected return".to_string()));
    }
    Err(Error::BlockReturn(outer.__irep_id, val))
}

pub(crate) fn op_break(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let val = vm.get_reg_value(a);
    // record where this break must land — the nearest
    // do_op_send crumb in the stack, captured BEFORE any unwinding or
    // intermediate error handling can pop crumbs. The crumb id stays valid
    // after pops, so the unwinder can tell when that frame is gone.
    let mut landing = None;
    for bc in vm.breadcrumbs.borrow().iter().rev() {
        if bc.event == "do_op_send" && bc.return_reg.is_some() {
            landing = Some((bc.id, bc.return_reg.unwrap_or(0)));
            break;
        }
    }
    // break outside any iterator (e.g. a Proc called outside
    // its loop) has no landing pad; raise LocalJumpError instead of unwinding
    // into a bogus target and unbalancing the crumb stack.
    if landing.is_none() {
        return Err(Error::LocalJumpError("unexpected break".to_string()));
    }
    vm.break_landing.replace(landing);
    Err(Error::Break(val))
}

pub(crate) fn op_blkpush(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, _s) = operand.as_bs()?;
    let n = vm.callinfo_stack.last().unwrap().n_args;
    let block = vm.get_current_regs_cloned(n + 1)?;
    vm.set_reg(a as usize, block);
    Ok(())
}

pub(crate) fn op_add(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let b = a + 1;
    let val1 = vm.current_regs()[a].clone();
    let val2 = vm.current_regs()[b].clone();
    let fast = match (&val1, &val2) {
        (Some(Value::Integer(n1)), Some(Value::Integer(n2))) => Some(Value::Integer(n1 + n2)),
        (Some(Value::Float(n1)), Some(Value::Float(n2))) => Some(Value::Float(n1 + n2)),
        (Some(Value::Integer(n1)), Some(Value::Float(n2))) => Some(Value::Float(*n1 as f64 + n2)),
        (Some(Value::Float(n1)), Some(Value::Integer(n2))) => Some(Value::Float(n1 + *n2 as f64)),
        (Some(Value::Object(o1)), Some(Value::Object(o2)))
            if matches!(o1.value, RValue::String(..)) && matches!(o2.value, RValue::String(..)) =>
        {
            let (RValue::String(s1, _), RValue::String(s2, _)) = (&o1.value, &o2.value) else {
                unreachable!("guarded String")
            };
            let mut bytes = s1.borrow().to_vec();
            bytes.extend_from_slice(&s2.borrow());
            Some(Value::Object(Rc::new(RObject::string_from_vec(bytes))))
        }
        _ => None,
    };
    if let Some(result) = fast {
        vm.current_regs()[a].replace(result);
        return Ok(());
    }
    let val1 = vm.current_regs()[a].clone().expect("op_add lhs");
    let val2 = vm.current_regs()[b].clone().expect("op_add rhs");
    let result = mrb_funcall(vm, Some(val1), "+", &[val2])?;
    vm.set_reg_value(a, result);
    Ok(())
}

pub(crate) fn op_addi(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let val1 = vm.current_regs()[a as usize].clone();
    let val2 = b as i64;
    let result = match &val1 {
        Some(Value::Integer(n1)) => Value::Integer(n1 + val2),
        Some(Value::Float(n1)) => Value::Float(n1 + val2 as f64),
        _ => {
            unreachable!("addi supports only integer and float")
        }
    };
    vm.current_regs()[a as usize].replace(result);
    Ok(())
}

pub(crate) fn op_sub(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let b = a + 1;
    let val1 = vm.current_regs()[a].clone();
    let val2 = vm.current_regs()[b].clone();
    let fast = match (&val1, &val2) {
        (Some(Value::Integer(n1)), Some(Value::Integer(n2))) => Some(Value::Integer(n1 - n2)),
        (Some(Value::Float(n1)), Some(Value::Float(n2))) => Some(Value::Float(n1 - n2)),
        (Some(Value::Integer(n1)), Some(Value::Float(n2))) => Some(Value::Float(*n1 as f64 - n2)),
        (Some(Value::Float(n1)), Some(Value::Integer(n2))) => Some(Value::Float(n1 - *n2 as f64)),
        _ => None,
    };
    if let Some(result) = fast {
        vm.current_regs()[a].replace(result);
        return Ok(());
    }
    let val1 = vm.current_regs()[a].clone().expect("op_sub lhs");
    let val2 = vm.current_regs()[b].clone().expect("op_sub rhs");
    let result = mrb_funcall(vm, Some(val1), "-", &[val2])?;
    vm.set_reg_value(a, result);
    Ok(())
}

pub(crate) fn op_subi(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let val1 = vm.current_regs()[a as usize].clone();
    let val2 = b as i64;
    let result = match &val1 {
        Some(Value::Integer(n1)) => Value::Integer(n1 - val2),
        _ => {
            unreachable!("subi supports only integer")
        }
    };
    vm.current_regs()[a as usize].replace(result);
    Ok(())
}

fn math_immediate_to_local(vm: &mut VM, operand: &Fetched, add: bool) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;
    let a = a as usize;
    let amount = if add { c as i64 } else { -(c as i64) };
    let value = vm.get_reg_value(a);
    let result = match &value {
        Value::Integer(n) => Value::Integer(n + amount),
        Value::Float(n) => Value::Float(n + amount as f64),
        _ => {
            // Other receivers are sent the method; the call runs in the window ops.h reserves at R[b].
            let arg = Value::Integer(c as i64);
            vm.current_regs_offset += b as usize;
            let res = mrb_funcall(vm, Some(value), if add { "+" } else { "-" }, &[arg]);
            vm.current_regs_offset -= b as usize;
            res?
        }
    };
    vm.current_regs()[a].replace(result);
    Ok(())
}

pub(crate) fn op_addilv(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    math_immediate_to_local(vm, operand, true)
}

pub(crate) fn op_subilv(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    math_immediate_to_local(vm, operand, false)
}

pub(crate) fn op_mul(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let b = a + 1;
    let val1 = vm.current_regs()[a].clone();
    let val2 = vm.current_regs()[b].clone();
    let fast = match (&val1, &val2) {
        (Some(Value::Integer(n1)), Some(Value::Integer(n2))) => Some(Value::Integer(n1 * n2)),
        (Some(Value::Float(n1)), Some(Value::Float(n2))) => Some(Value::Float(n1 * n2)),
        (Some(Value::Integer(n1)), Some(Value::Float(n2))) => Some(Value::Float(*n1 as f64 * n2)),
        (Some(Value::Float(n1)), Some(Value::Integer(n2))) => Some(Value::Float(n1 * *n2 as f64)),
        _ => None,
    };
    if let Some(result) = fast {
        vm.current_regs()[a].replace(result);
        return Ok(());
    }
    let val1 = vm.current_regs()[a].clone().expect("op_mul lhs");
    let val2 = vm.current_regs()[b].clone().expect("op_mul rhs");
    let result = mrb_funcall(vm, Some(val1), "*", &[val2])?;
    vm.set_reg_value(a, result);
    Ok(())
}

pub(crate) fn op_div(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let b = a + 1;
    let val1 = vm.current_regs()[a].clone();
    let val2 = vm.current_regs()[b].clone();
    let fast = match (&val1, &val2) {
        (Some(Value::Integer(n1)), Some(Value::Integer(n2))) => Some(Value::Integer(n1 / n2)),
        (Some(Value::Float(n1)), Some(Value::Float(n2))) => Some(Value::Float(n1 / n2)),
        (Some(Value::Integer(n1)), Some(Value::Float(n2))) => Some(Value::Float(*n1 as f64 / n2)),
        (Some(Value::Float(n1)), Some(Value::Integer(n2))) => Some(Value::Float(n1 / *n2 as f64)),
        _ => None,
    };
    if let Some(result) = fast {
        vm.current_regs()[a].replace(result);
        return Ok(());
    }
    let val1 = vm.current_regs()[a].clone().expect("op_div lhs");
    let val2 = vm.current_regs()[b].clone().expect("op_div rhs");
    let result = mrb_funcall(vm, Some(val1), "/", &[val2])?;
    vm.set_reg_value(a, result);
    Ok(())
}

/// Falls back to <=> dispatch for non-numeric operands, mirroring op_div.
fn compare_via_spaceship(vm: &mut VM, val1: Value, val2: Value, op: &str) -> Result<Value, Error> {
    let r = mrb_funcall(vm, Some(val1), "<=>", &[val2])?;
    match &r {
        Value::Nil => Err(Error::ArgumentError("comparison failed".into())),
        _ => {
            let ord =
                i64::try_from(&r).map_err(|_| Error::ArgumentError("bad <=> result".into()))?;
            let hit = match op {
                "<" => ord < 0,
                "<=" => ord <= 0,
                ">" => ord > 0,
                _ => ord >= 0,
            };
            Ok(Value::Bool(hit))
        }
    }
}

pub(crate) fn op_lt(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let b = a + 1;
    let val1 = vm.current_regs()[a].clone();
    let val2 = vm.current_regs()[b].clone();
    let fast = match (&val1, &val2) {
        (Some(Value::Integer(n1)), Some(Value::Integer(n2))) => Some(n1 < n2),
        (Some(Value::Float(n1)), Some(Value::Float(n2))) => Some(n1 < n2),
        (Some(Value::Integer(n1)), Some(Value::Float(n2))) => Some((*n1 as f64) < *n2),
        (Some(Value::Float(n1)), Some(Value::Integer(n2))) => Some(*n1 < (*n2 as f64)),
        _ => None,
    };
    if let Some(result) = fast {
        vm.current_regs()[a].replace(Value::Bool(result));
        return Ok(());
    }
    let val1 = vm.current_regs()[a].clone().expect("op_lt lhs");
    let val2 = vm.current_regs()[b].clone().expect("op_lt rhs");
    let result = compare_via_spaceship(vm, val1, val2, "<")?;
    vm.set_reg_value(a, result);
    Ok(())
}

pub(crate) fn op_le(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let b = a + 1;
    let val1 = vm.current_regs()[a].clone();
    let val2 = vm.current_regs()[b].clone();
    let fast = match (&val1, &val2) {
        (Some(Value::Integer(n1)), Some(Value::Integer(n2))) => Some(n1 <= n2),
        (Some(Value::Float(n1)), Some(Value::Float(n2))) => Some(n1 <= n2),
        (Some(Value::Integer(n1)), Some(Value::Float(n2))) => Some((*n1 as f64) <= *n2),
        (Some(Value::Float(n1)), Some(Value::Integer(n2))) => Some(*n1 <= (*n2 as f64)),
        _ => None,
    };
    if let Some(result) = fast {
        vm.current_regs()[a].replace(Value::Bool(result));
        return Ok(());
    }
    let val1 = vm.current_regs()[a].clone().expect("op_le lhs");
    let val2 = vm.current_regs()[b].clone().expect("op_le rhs");
    let result = compare_via_spaceship(vm, val1, val2, "<=")?;
    vm.set_reg_value(a, result);
    Ok(())
}

pub(crate) fn op_eq(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let b = a + 1;
    let val1 = vm.current_regs()[a].clone();
    let val2 = vm.current_regs()[b].clone();
    // Object#== semantics for immediates (via ValueEquality): same-kind values
    // compare by value; Integer and Float are never equal across kinds.
    let fast = match (&val1, &val2) {
        (Some(Value::Integer(n1)), Some(Value::Integer(n2))) => Some(n1 == n2),
        (Some(Value::Float(n1)), Some(Value::Float(n2))) => Some(n1 == n2),
        (Some(Value::Bool(x)), Some(Value::Bool(y))) => Some(x == y),
        (Some(Value::Nil), Some(Value::Nil)) => Some(true),
        (Some(Value::Symbol(x)), Some(Value::Symbol(y))) => Some(x == y),
        _ => None,
    };
    if let Some(equal) = fast {
        vm.current_regs()[a].replace(Value::Bool(equal));
        return Ok(());
    }
    let lhs = vm.current_regs()[a].clone().expect("op_eq lhs");
    let rhs = vm.current_regs()[b].clone().expect("op_eq rhs");
    let result = mrb_object_is_equal(vm, lhs, rhs);
    vm.set_reg_value(a, result);
    Ok(())
}

pub(crate) fn op_gt(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let b = a + 1;
    let val1 = vm.current_regs()[a].clone();
    let val2 = vm.current_regs()[b].clone();
    let fast = match (&val1, &val2) {
        (Some(Value::Integer(n1)), Some(Value::Integer(n2))) => Some(n1 > n2),
        (Some(Value::Float(n1)), Some(Value::Float(n2))) => Some(n1 > n2),
        (Some(Value::Integer(n1)), Some(Value::Float(n2))) => Some((*n1 as f64) > *n2),
        (Some(Value::Float(n1)), Some(Value::Integer(n2))) => Some(*n1 > (*n2 as f64)),
        _ => None,
    };
    if let Some(result) = fast {
        vm.current_regs()[a].replace(Value::Bool(result));
        return Ok(());
    }
    let val1 = vm.current_regs()[a].clone().expect("op_gt lhs");
    let val2 = vm.current_regs()[b].clone().expect("op_gt rhs");
    let result = compare_via_spaceship(vm, val1, val2, ">")?;
    vm.set_reg_value(a, result);
    Ok(())
}

pub(crate) fn op_ge(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let b = a + 1;
    let val1 = vm.current_regs()[a].clone();
    let val2 = vm.current_regs()[b].clone();
    let fast = match (&val1, &val2) {
        (Some(Value::Integer(n1)), Some(Value::Integer(n2))) => Some(n1 >= n2),
        (Some(Value::Float(n1)), Some(Value::Float(n2))) => Some(n1 >= n2),
        (Some(Value::Integer(n1)), Some(Value::Float(n2))) => Some((*n1 as f64) >= *n2),
        (Some(Value::Float(n1)), Some(Value::Integer(n2))) => Some(*n1 >= (*n2 as f64)),
        _ => None,
    };
    if let Some(result) = fast {
        vm.current_regs()[a].replace(Value::Bool(result));
        return Ok(());
    }
    let val1 = vm.current_regs()[a].clone().expect("op_ge lhs");
    let val2 = vm.current_regs()[b].clone().expect("op_ge rhs");
    let result = compare_via_spaceship(vm, val1, val2, ">=")?;
    vm.set_reg_value(a, result);
    Ok(())
}

pub(crate) fn op_array(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    do_op_array(vm, a as usize, a as usize, b as usize)
}

pub(crate) fn op_array2(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;
    do_op_array(vm, a as usize, b as usize, c as usize)
}

fn do_op_array(vm: &mut VM, this: usize, start: usize, n: usize) -> Result<(), Error> {
    let mut ary = Vec::with_capacity(n);
    for i in 0..n {
        if this == start && i == 0 {
            ary.push(vm.take_reg_value(start));
        } else {
            ary.push(vm.get_reg_value(start + i));
        }
    }
    let val = RObject::array(ary);
    vm.set_reg(this, val.to_refcount_assigned());
    Ok(())
}

pub(crate) fn op_arycat(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let b = a + 1;
    let val1 = vm.get_current_regs_cloned(a)?;
    let val2 = vm.take_current_regs(b)?;
    match (&val1.value, &val2.value) {
        (RValue::Array(ary1), RValue::Array(ary2)) => {
            let mut ary1 = ary1.borrow_mut();
            let ary2 = ary2.borrow();
            for item in ary2.iter() {
                ary1.push(item.clone());
            }
        }
        (RValue::Nil, RValue::Array(ary2)) => {
            let mut ary1 = Vec::new();
            let ary2 = ary2.borrow();
            for item in ary2.iter() {
                ary1.push(item.clone());
            }
            let val = RObject::array(ary1);
            vm.set_reg(a, val.to_refcount_assigned());
        }
        _ => {
            unreachable!("arycat supports only array")
        }
    };
    Ok(())
}

pub(crate) fn op_arypush(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let a = a as usize;
    let b = b as usize;

    let items: Vec<Value> = (0..b).map(|i| vm.take_reg_value(a + 1 + i)).collect();

    let ary = vm.get_current_regs_cloned(a)?;
    match &ary.value {
        RValue::Array(_) => {
            let mut inner = ary.array_borrow_mut()?;
            inner.extend(items);
        }
        RValue::Nil => {
            let val = RObject::array(items);
            vm.set_reg(a, val.to_refcount_assigned());
        }
        _ => {
            unreachable!("arypush supports only array")
        }
    }
    Ok(())
}

pub(crate) fn op_arysplat(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let val = vm.get_current_regs_cloned(a)?;
    match &val.value {
        RValue::Array(_) => {}
        RValue::Nil => {
            let ary = RObject::array(Vec::new());
            vm.set_reg(a, ary.to_refcount_assigned());
        }
        _ => {
            let ary = RObject::array(vec![Value::from_rc(val)]);
            vm.set_reg(a, ary.to_refcount_assigned());
        }
    }
    Ok(())
}

pub(crate) fn op_aref(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;
    let array = vm.get_current_regs_cloned(b as usize)?;
    let index = c as usize;
    match &array.value {
        RValue::Array(ary) => {
            let ary = ary.borrow();
            let val = ary.get(index).cloned().unwrap_or(Value::Nil);
            vm.set_reg_value(a as usize, val);
        }
        _ => {
            unreachable!("aref supports only array")
        }
    };
    Ok(())
}

pub(crate) fn op_apost(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;
    if c != 0 {
        return Err(Error::internal(
            "apost with 3 operands is not supported yet",
        ));
    }
    let array = vm.get_current_regs_cloned(a as usize)?;
    let n = b as usize;
    match &array.value {
        RValue::Array(ary) => {
            let mut dest = Vec::new();
            let ary = ary.borrow();
            for i in n..ary.len() {
                dest.push(ary[i].clone());
            }
            let newval = RObject::array(dest).to_refcount_assigned();
            vm.set_reg(a as usize, newval);
        }
        _ => {
            unreachable!("apost supports only array")
        }
    };
    Ok(())
}

pub(crate) fn op_symbol(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let symstr = vm.current_irep.pool[b as usize].as_str().to_string();
    vm.current_regs()[a as usize].replace(Value::Symbol(intern_symbol(&symstr)));
    Ok(())
}

pub(crate) fn op_string(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let str = vm.current_irep.pool[b as usize].as_str().to_string();
    let val = RObject::string(str);
    vm.set_reg(a as usize, val.to_refcount_assigned());
    Ok(())
}

pub(crate) fn op_strcat(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let b = a + 1;
    let val1 = vm.get_reg_value(a);
    let val2 = vm.get_reg_value(b);
    let ra = match &val1 {
        Value::Object(a) => a,
        _ => unreachable!("strcat supports only string"),
    };
    let s1 = match &ra.value {
        RValue::String(s, _) => s,
        _ => unreachable!("strcat supports only string"),
    };
    // Append by memcpy (not per-byte push) and without boxing the right
    // operand. `to_s` only runs for operands that are not already a String or
    // Integer.
    match &val2 {
        Value::Object(o2) => match &o2.value {
            RValue::String(s2, _) => {
                if Rc::ptr_eq(ra, o2) {
                    let bytes = s2.borrow().clone();
                    s1.borrow_mut().extend_from_slice(&bytes);
                } else {
                    let s2 = s2.borrow();
                    s1.borrow_mut().extend_from_slice(&s2);
                }
            }
            _ => append_to_string(vm, s1, val2)?,
        },
        Value::Integer(i) => {
            s1.borrow_mut().extend_from_slice(i.to_string().as_bytes());
        }
        _ => append_to_string(vm, s1, val2)?,
    }
    Ok(())
}

/// Slow path of `OP_STRCAT`: coerce the right operand through `to_s`.
fn append_to_string(vm: &mut VM, s1: &RefCell<Vec<u8>>, val2: Value) -> Result<(), Error> {
    let s2 = mrb_funcall(vm, Some(val2), "to_s", &[])?;
    let bytes = match &s2 {
        Value::Object(o) => match &o.value {
            RValue::String(s, _) => s.borrow().to_vec(),
            _ => unreachable!("to_s must return string"),
        },
        _ => unreachable!("to_s must return string"),
    };
    s1.borrow_mut().extend_from_slice(&bytes);
    Ok(())
}

pub(crate) fn op_hash(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let a = a as usize;
    let b = b as usize;
    let mut hash = RHashMap::default();
    for i in 0..b {
        let key = vm.get_reg_value(a + i * 2);
        let val = vm.get_reg_value(a + i * 2 + 1);
        hash.insert(key.as_hash_key()?, (key, val));
    }
    let val = RObject::hash(hash);
    vm.set_reg(a, Rc::new(val));
    Ok(())
}

pub(crate) fn op_hashadd(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let a = a as usize;
    let b = b as usize;

    let pairs: Vec<(Value, Value)> = (0..b)
        .map(|i| {
            let key = vm.take_reg_value(a + i * 2 + 1);
            let val = vm.take_reg_value(a + i * 2 + 2);
            (key, val)
        })
        .collect();

    let hash = vm.get_current_regs_cloned(a)?;
    let mut inner = hash.hash_borrow_mut()?;
    for (key, val) in pairs {
        let hashed = key.as_hash_key()?;
        inner.insert(hashed, (key, val));
    }
    Ok(())
}

pub(crate) fn op_hashcat(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let other = vm.take_current_regs(a + 1)?;

    let hash = vm.get_current_regs_cloned(a)?;
    match (&hash.value, &other.value) {
        (RValue::Hash(_), RValue::Hash(other_hash)) => {
            let mut inner = hash.hash_borrow_mut()?;
            for (hashed, (key, val)) in other_hash.borrow().iter() {
                inner.insert(hashed.clone(), (key.clone(), val.clone()));
            }
        }
        (RValue::Nil, RValue::Hash(other_hash)) => {
            let mut fresh = RHashMap::default();
            for (hashed, (key, val)) in other_hash.borrow().iter() {
                fresh.insert(hashed.clone(), (key.clone(), val.clone()));
            }
            let val = RObject::hash(fresh);
            vm.set_reg(a, val.to_refcount_assigned());
        }
        _ => {
            return Err(Error::TypeMismatch);
        }
    }
    Ok(())
}

pub(crate) fn op_lambda(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let irep = Some(vm.current_irep.reps[b as usize].clone());
    let environ = ENV {
        __irep_id: vm.current_irep.__id,
        upper: vm.upper.clone(),
        current_regs_offset: vm.current_regs_offset,
        is_expired: Cell::new(false),
        captured: RefCell::new(None),
        is_lambda: Cell::new(true),
        closure_irep_id: irep.as_ref().unwrap().__id,
    };
    //let nregs = vm.current_irep.nregs;
    //environ.capture(&vm.current_regs()[0..nregs]);
    let environ = Rc::new(environ);
    vm.cur_env.insert(vm.current_irep.__id, environ.clone());
    vm.has_env_ref.insert(vm.current_irep.__id, true);

    let val = RObject {
        tt: RType::Proc,
        value: RValue::Proc(RProc {
            irep,
            is_rb_func: true,
            is_fnblock: false,
            sym_id: Some(intern_symbol("<lambda>")),
            next: None,
            func: None,
            environ: Some(environ),
            block_self: Some(vm.getself()?),
            fast_op: None,
            attr_key: None,
        }),
        object_id: u64::MAX.into(),
        singleton_class: RefCell::new(None),
        ivar: RefCell::new(IvarMap::new()),
    };
    vm.set_reg(a as usize, val.to_refcount_assigned());
    Ok(())
}

pub(crate) fn op_block(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let irep = Some(vm.current_irep.reps[b as usize].clone());
    let environ = ENV {
        __irep_id: vm.current_irep.__id,
        upper: vm.upper.clone(),
        current_regs_offset: vm.current_regs_offset,
        is_expired: Cell::new(false),
        captured: RefCell::new(None),
        is_lambda: Cell::new(false),
        closure_irep_id: irep.as_ref().unwrap().__id,
    };
    let environ = Rc::new(environ);
    vm.cur_env.insert(vm.current_irep.__id, environ.clone());
    vm.has_env_ref.insert(vm.current_irep.__id, true);

    let val = RObject {
        tt: RType::Proc,
        value: RValue::Proc(RProc {
            irep,
            is_rb_func: true,
            is_fnblock: false,
            sym_id: Some(intern_symbol("<block>")),
            next: None,
            func: None,
            environ: Some(environ),
            block_self: Some(vm.getself()?),
            fast_op: None,
            attr_key: None,
        }),
        object_id: u64::MAX.into(),
        singleton_class: RefCell::new(None),
        ivar: RefCell::new(IvarMap::new()),
    };
    vm.set_reg(a as usize, val.to_refcount_assigned());
    Ok(())
}

pub(crate) fn op_method(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let irep = Some(vm.current_irep.reps[b as usize].clone());
    let val = RObject {
        tt: super::value::RType::Proc,
        value: super::value::RValue::Proc(super::value::RProc {
            irep,
            is_rb_func: true,
            is_fnblock: false,
            sym_id: None,
            next: None,
            func: None,
            environ: None,
            block_self: None,
            fast_op: None,
            attr_key: None,
        }),
        object_id: u64::MAX.into(),
        singleton_class: RefCell::new(None),
        ivar: RefCell::new(IvarMap::new()),
    };
    vm.set_reg(a as usize, val.to_refcount_assigned());
    Ok(())
}

pub(crate) fn op_range_inc(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()?;
    do_op_range(vm, a as usize, a as usize + 1, false)
}

pub(crate) fn op_range_exc(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()?;
    do_op_range(vm, a as usize, a as usize + 1, true)
}

fn do_op_range(vm: &mut VM, a: usize, b: usize, exclusive: bool) -> Result<(), Error> {
    let val1 = vm.get_reg_value(a);
    let val2 = vm.get_reg_value(b);
    let val = RObject::range(val1, val2, exclusive);
    vm.set_reg(a, val.to_refcount_assigned());
    Ok(())
}

pub(crate) fn op_oclass(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let val = RObject::class(vm.object_class.clone(), vm);
    vm.set_reg(a, val);
    Ok(())
}

fn current_namespace(vm: &mut VM) -> Option<Rc<RModule>> {
    let obj = vm.current_regs()[0].as_ref()?;
    match obj.rvalue() {
        Some(RValue::Class(klass)) => Some(klass.module.clone()),
        Some(RValue::Module(module)) => Some(module.clone()),
        _ => None,
    }
}

pub(crate) fn op_class(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let superclass = vm.current_regs()[a as usize + 1].as_ref().cloned();
    let name = vm.current_irep.syms[b as usize].clone();

    // Reuse an existing class wrapper instead of replacing it, so singleton
    // methods registered by native code survive a reopen.
    // Scope chain only (current namespace, then top level): a same-named
    // class in an unrelated module must not hijack this definition, and a
    // cross-scope reuse must also bind the constant in the current one.
    let lookup_key = name.name.clone();
    let mut reused: Option<Rc<RObject>> = None;
    let mut scopes: Vec<Option<Rc<RModule>>> =
        vec![current_namespace(vm), Some(vm.object_class.module.clone())];
    for ns in scopes.drain(..).flatten() {
        // Clone out and drop the Ref guard before any borrow_mut on the
        // same consts: reopening an existing class hits this path with
        // cur == ns.
        let found = ns
            .consts
            .borrow()
            .get(&lookup_key)
            .cloned()
            .filter(|v| matches!(v.value, RValue::Class(_)));
        if let Some(existing) = found {
            if let Some(cur) = current_namespace(vm) {
                cur.consts
                    .borrow_mut()
                    .insert(lookup_key.clone(), existing.clone());
                vm.bump_const_version();
            }
            reused = Some(existing);
            break;
        }
    }
    if let Some(existing) = reused {
        vm.set_reg(a as usize, existing);
        return Ok(());
    }

    let superclass = match superclass {
        Some(superclass) => {
            if let RValue::Class(klass) = &superclass.to_rc().value {
                klass.clone()
            } else {
                vm.object_class.clone()
            }
        }
        None => vm.object_class.clone(),
    };
    let parent_module = current_namespace(vm);
    let name = name.name;
    let klass = vm.define_class(&name, Some(superclass), parent_module.clone());

    // register constant under parent namespace (if any) or top-level
    let class_value = RObject::class(klass.clone(), vm);
    class_value.initialize_or_get_singleton_class_for_class(vm);
    if let Some(parent) = parent_module {
        parent
            .consts
            .borrow_mut()
            .insert(name.clone(), class_value.clone());
    } else {
        vm.consts.insert(name.clone(), class_value.clone());
    }

    vm.set_reg(a as usize, class_value);
    Ok(())
}

pub(crate) fn op_module(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let name = vm.current_irep.syms[b as usize].clone();

    // Reuse an existing module wrapper instead of replacing it, so singleton
    // methods registered by native code survive a reopen.
    let lookup_key = name.name.clone();
    let search_scopes: Vec<Option<Rc<RModule>>> =
        vec![current_namespace(vm), Some(vm.object_class.module.clone())];
    for scope in &search_scopes {
        if let Some(ns) = scope
            && let Some(existing) = ns.consts.borrow().get(&lookup_key).cloned()
            && let RValue::Module(ref _m) = existing.value
        {
            vm.set_reg(a as usize, existing);
            return Ok(());
        }
    }
    if let Some(existing) = vm.get_const_by_name(&lookup_key)
        && let RValue::Module(_) = existing.value
    {
        vm.set_reg(a as usize, existing);
        return Ok(());
    }

    let name = name.name;
    let parent_module = current_namespace(vm);
    let module = vm.define_module(&name, parent_module.clone());

    // one canonical wrapper shared by consts and the body
    // self register; two wrappers made body-self singletons invisible to
    // constant lookups.
    let module_value = Rc::new(RObject::module(module.clone()));
    if let Some(parent) = parent_module {
        parent
            .consts
            .borrow_mut()
            .insert(name.clone(), module_value.clone());
    } else {
        vm.consts.insert(name.clone(), module_value.clone());
    }

    vm.set_reg(a as usize, module_value);
    Ok(())
}

pub(crate) fn op_exec(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let recv = vm.get_current_regs_cloned(a as usize)?;
    // guard the child irep's register window before pushing
    // its frame; unbounded recursion must raise SystemStackError, not panic.
    let irep = vm.current_irep.reps[b as usize].clone();
    vm.check_frame_window(a as usize, irep.nregs)?;

    vm.push_breadcrumb(
        "exec",
        Some(CallerLabel::Static("<exec>")),
        None,
        Some(vm.current_irep.clone()),
        Some(vm.pc.get().saturating_sub(1)),
    );
    push_callinfo(vm, intern_symbol("<exec>"), 0, None, a as usize, false);

    vm.pc.set(0);
    vm.current_irep = irep;
    vm.current_regs_offset += a as usize;

    // If recv is a Class or Module, set target_class accordingly
    vm.target_class = match &recv.value {
        RValue::Class(klass) => TargetContext::Class(klass.clone()),
        RValue::Module(module) => TargetContext::Module(module.clone()),
        _ => TargetContext::Class(recv.get_class(vm)),
    };
    Ok(())
}

pub(crate) fn op_def(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let target = vm.get_current_regs_cloned(a as usize)?;
    let method = vm.get_current_regs_cloned(a as usize + 1)?;
    let sym = vm.current_irep.syms[b as usize].clone();

    let method_ref = method.as_ref();

    // First, extract and prepare the method from the Proc
    let method = match &method_ref.value {
        RValue::Proc(proc) => {
            let mut method = proc.clone();
            method.environ = None; // method cannot trace the upper environment
            method.sym_id = Some(sym.id);
            Ok(method)
        }
        _ => Err(Error::ArgumentError(
            "def operand 2 must be Proc (method)".to_string(),
        )),
    }?;

    // Then, define it on the receiver
    let target_ref = target.as_ref();
    match &target_ref.value {
        RValue::Class(klass) => {
            let mut procs = klass.procs.borrow_mut();
            procs.insert(sym.id, method);
        }
        RValue::Module(module) => {
            let mut procs = module.procs.borrow_mut();
            procs.insert(sym.id, method);
        }
        _ => {
            let robject = target.clone();
            let current_class = robject.get_class(vm);
            let sclass = if current_class.is_singleton {
                current_class
            } else {
                robject.initialize_or_get_singleton_class(vm)
            };
            let mut procs = sclass.procs.borrow_mut();
            procs.insert(sym.id, method);
        }
    }
    vm.bump_method_version();
    vm.set_reg(a as usize, RObject::symbol(sym).to_refcount_assigned());
    Ok(())
}

pub(crate) fn op_alias(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b) = operand.as_bb()?;
    let new_name = vm.current_irep.syms[a as usize].clone();
    let old_name = vm.current_irep.syms[b as usize].clone();

    let owner = vm.target_class.clone();
    let (owner_module, method) = match &owner {
        TargetContext::Class(klass) => {
            let (owner_module, method) = resolve_method(klass, &old_name.name)
                .ok_or_else(|| Error::NoMethodError(old_name.name.clone()))?;
            (owner_module, method)
        }
        TargetContext::Module(module) => {
            let method = module
                .procs
                .borrow()
                .get(&old_name.id)
                .cloned()
                .ok_or_else(|| Error::NoMethodError(old_name.name.clone()))?;
            (module.clone(), method)
        }
    };

    let mut new_method = method.clone();
    new_method.sym_id = Some(new_name.id);

    let mut procs = owner_module.procs.borrow_mut();
    procs.insert(new_name.id, new_method);
    vm.bump_method_version();

    Ok(())
}

pub(crate) fn op_undef(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()?;
    let sym = vm.current_irep.syms[a as usize].clone();

    let owner = vm.target_class.clone();
    match &owner {
        TargetContext::Class(klass) => {
            let mut procs = klass.procs.borrow_mut();
            procs.remove(&sym.id);
        }
        TargetContext::Module(module) => {
            let mut procs = module.procs.borrow_mut();
            procs.remove(&sym.id);
        }
    };
    vm.bump_method_version();
    Ok(())
}

pub(crate) fn op_sclass(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let val = vm.current_regs()[a]
        .take()
        .expect("SCLASS: operand too short")
        .to_rc();
    let singleton_class = match val.tt {
        RType::Class | RType::Module => val.initialize_or_get_singleton_class_for_class(vm),
        _ => val.initialize_or_get_singleton_class(vm),
    };
    let robj = RObject::class(singleton_class.clone(), vm);
    vm.set_reg(a, robj);
    Ok(())
}

pub(crate) fn op_tclass(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let a = operand.as_b()? as usize;
    let val: Rc<RObject> = match &vm.target_class {
        TargetContext::Class(klass) => RObject::class(klass.clone(), vm),
        TargetContext::Module(module) => Rc::new(module.clone().into()),
    };
    vm.set_reg(a, val);
    Ok(())
}

fn method_from_irep(vm: &VM, index: usize, sym: RSym) -> RProc {
    RProc {
        is_rb_func: true,
        is_fnblock: false,
        sym_id: Some(sym.id),
        next: None,
        irep: Some(vm.current_irep.reps[index].clone()),
        func: None,
        environ: None,
        block_self: None,
        fast_op: None,
        attr_key: None,
    }
}

pub(crate) fn op_tdef(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;
    let sym = vm.current_irep.syms[b as usize].clone();
    let method = method_from_irep(vm, c as usize, sym.clone());
    match vm.target_class.clone() {
        TargetContext::Class(klass) => {
            klass.procs.borrow_mut().insert(sym.id, method);
        }
        TargetContext::Module(module) => {
            module.procs.borrow_mut().insert(sym.id, method);
        }
    }
    vm.bump_method_version();
    vm.current_regs()[a as usize]
        .replace(Value::from_rc(RObject::symbol(sym).to_refcount_assigned()));
    Ok(())
}

pub(crate) fn op_sdef(vm: &mut VM, operand: &Fetched) -> Result<(), Error> {
    let (a, b, c) = operand.as_bbb()?;
    let sym = vm.current_irep.syms[b as usize].clone();
    let method = method_from_irep(vm, c as usize, sym.clone());
    let target = vm.get_current_regs_cloned(a as usize)?;
    let singleton = match target.tt {
        RType::Class | RType::Module => target.initialize_or_get_singleton_class_for_class(vm),
        _ => target.initialize_or_get_singleton_class(vm),
    };
    singleton.procs.borrow_mut().insert(sym.id, method);
    vm.bump_method_version();
    vm.current_regs()[a as usize]
        .replace(Value::from_rc(RObject::symbol(sym).to_refcount_assigned()));
    Ok(())
}

pub(crate) fn op_stop(vm: &mut VM, _operand: &Fetched) -> Result<(), Error> {
    vm.flag_preemption.set(true);
    Ok(())
}
