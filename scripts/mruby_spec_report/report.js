const files = [...document.querySelectorAll('.file')];
const search = document.querySelector('#file-search');
const filter = document.querySelector('#file-filter');
const assertions = document.querySelector('#assert-filter');
document.querySelector('.filters').hidden = false;
function update() {
  let visible = 0;
  for (const file of files) {
    const d = file.dataset;
    const matches = filter.value === 'all' ||
      (filter.value === 'passed' && d.status === 'passed') ||
      (filter.value === 'failures' && d.failures === 'true') ||
      (filter.value === 'stopped' && d.stopped === 'true') ||
      (filter.value === 'attention' && d.status !== 'passed');
    file.hidden = !matches || !d.name.includes(search.value.trim().toLowerCase());
    if (!file.hidden) visible++;
    const rows = [...file.querySelectorAll('tbody tr')];
    rows.forEach(row => { row.hidden = assertions.checked && row.dataset.result === 'pass'; });
    file.querySelector('.no-assertions').hidden = rows.some(row => !row.hidden);
  }
  document.querySelector('#visible-count').textContent = `${visible} of ${files.length} files`;
  document.querySelector('#empty-state').hidden = visible !== 0;
}
search.addEventListener('input', update);
filter.addEventListener('change', update);
assertions.addEventListener('change', update);
function openAnchor() {
  const file = files.find(file => `#${file.id}` === location.hash);
  if (file) {
    search.value = '';
    filter.value = 'all';
    update();
    file.open = true;
    file.scrollIntoView();
  }
}
window.addEventListener('hashchange', openAnchor);
update();
openAnchor();
