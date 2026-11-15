// Standalone frontend comparisons. Working functionality is a separate measurement.
const referenceParityDashboard = (() => {
  const states = new Set(['not_run','passed','failed','stale','invalid','missing','unsupported']);
  const count = n => Number.isSafeInteger(n) && n >= 0;
  const hash = s => typeof s === 'string' && /^[0-9a-f]{64}$/.test(s);
  const fraction = n => Number.isFinite(n) && n >= 0 && n <= 1;
  const metrics = m => m && count(m.pixels) && m.pixels > 0 && m.pixels <= 4096*4096
    && Number.isFinite(m.mae) && m.mae >= 0 && m.mae <= 255
    && count(m.max_channel_error) && m.max_channel_error <= 255
    && fraction(m.bad_pixel_fraction) && fraction(m.reference_nonblack_fraction)
    && fraction(m.candidate_nonblack_fraction)
    && count(m.reference_spatial_range) && m.reference_spatial_range <= 255
    && count(m.candidate_spatial_range) && m.candidate_spatial_range <= 255;
  function valid(v) {
    if (!v || v.schema_version !== 1 || v.comparator !== 'png_rgba8_image_set_v1'
      || !states.has(v.state) || !count(v.total) || v.total < 1 || v.total > 128
      || !count(v.verified) || v.verified > v.total || !hash(v.case_set_sha256)
      || !Array.isArray(v.cases) || v.cases.length !== v.total
      || v.run_id !== null && !hash(v.run_id)) return false;
    const ids = new Set();
    for (const c of v.cases) {
      if (typeof c.id !== 'string' || ids.has(c.id) || typeof c.title !== 'string'
        || !states.has(c.state) || !count(c.passed_samples) || !count(c.total_samples)
        || c.total_samples < 1 || c.total_samples > 512 || c.passed_samples > c.total_samples
        || !Array.isArray(c.samples) || c.samples.length > c.total_samples) return false;
      ids.add(c.id);
      const samples = new Set();
      for (const s of c.samples) {
        if (typeof s.id !== 'string' || samples.has(s.id) || typeof s.passed !== 'boolean'
          || s.metrics !== null && !metrics(s.metrics) || s.passed && (!s.metrics || s.reason !== null)) return false;
        samples.add(s.id);
      }
      if (c.samples.filter(s => s.passed).length !== c.passed_samples) return false;
      if (c.state === 'passed' && (c.passed_samples !== c.total_samples
        || c.samples.length !== c.total_samples || c.samples.some(s => s.passed !== true || !s.metrics))) return false;
    }
    const current = v.cases.filter(c => c.state === 'passed').length;
    return current === v.verified && Number.isFinite(v.percent)
      && Math.abs(v.percent - 100 * current / v.total) < 1e-8
      && (!['stale','invalid','not_run'].includes(v.state) || current === 0)
      && (v.state !== 'passed' || current === v.total);
  }
  function render(value, {text, $, el, badge}) {
    const usable = valid(value);
    const status = value === undefined ? 'not_run' : usable ? value.state : 'invalid';
    $('reference-state').append(badge(status));
    text('reference-number', usable ? `${value.percent.toFixed(1)}%` : '—');
    text('reference-count', usable ? `${value.verified} of ${value.total} current frontend reference cases verified` : 'No trustworthy current comparison count');
    text('reference-summary', usable
      ? `Each case requires all of its declared samples. ${status === 'stale' ? 'The last result is stale; its historical metrics do not count as current verification. ' : ''}Code, executable, case definitions, reference images, probe images, settings and input contracts are checked again when the report refreshes. This measures the named frontend cases, not whole-game completion.`
      : 'Run the reference comparator with local original-game and Rust captures. Startup/menu functionality checks alone do not establish original-game parity.');
    if (!usable) return;
    text('reference-run', value.run_id ? `Comparison ${value.run_id.slice(0,12)} · case set ${value.case_set_sha256.slice(0,12)}` : `Case set ${value.case_set_sha256.slice(0,12)} · no comparison run yet`);
    for (const c of value.cases) {
      const row = el('div',undefined,'gate');
      const details = el('div');
      details.append(el('div',c.title));
      details.append(el('div',`${c.passed_samples}/${c.total_samples} samples${status === 'stale' ? ' in the previous measurement' : ''}`,'meta'));
      const reasons = [...new Set(c.samples.filter(s => s.reason).map(s => s.reason))];
      if (c.reason && reasons.length === 0) reasons.push(c.reason);
      if (reasons.length) details.append(el('div',reasons.map(s => String(s).replaceAll('_',' ')).join(' · '),'meta'));
      const measured = c.samples.find(s => s.metrics)?.metrics;
      if (measured && Number.isFinite(measured.mae) && Number.isFinite(measured.bad_pixel_fraction))
        details.append(el('div',`Sample RGBA error ${measured.mae.toFixed(3)} / 255; ${(100*measured.bad_pixel_fraction).toFixed(2)}% of pixels exceed the declared delta`,'meta'));
      row.append(details,badge(c.state)); $('reference-cases').append(row);
    }
  }
  return {valid,render};
})();
