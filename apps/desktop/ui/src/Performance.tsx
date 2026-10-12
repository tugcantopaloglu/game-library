import { useEffect, useState } from 'react';
import { Activity, ArrowUpRight, Clock3, Cpu, FolderOpen, HardDrive, LoaderCircle, Play, Square } from 'lucide-react';
import { command, desktop, errorText, subscribe } from './api';
import type { MeasurementReport, ResourceSample } from './bindings';

const number = (value: number | null | undefined, decimals = 1) => value == null ? '—' : value.toLocaleString('tr-TR', { minimumFractionDigits: decimals, maximumFractionDigits: decimals });

export function Performance({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const [sample, setSample] = useState<ResourceSample | null>(null);
  const [measuring, setMeasuring] = useState(false);
  const [report, setReport] = useState<MeasurementReport | null>(null);
  const [samples, setSamples] = useState<ResourceSample[]>([]);

  useEffect(() => {
    let active = true;
    void command<[boolean, MeasurementReport | null]>('measurement_status').then(([running, saved]) => { if (active) { setMeasuring(running); setReport(saved); } }).catch(error => { if (desktop) notify(errorText(error), true); });
    const listeners = [
      subscribe<ResourceSample>('measurement-sample', value => { if (active) { setSample(value); setMeasuring(true); setSamples(previous => [...previous.slice(-119), value]); } }),
      subscribe<MeasurementReport>('measurement-finished', value => { if (active) { setReport(value); setMeasuring(false); notify('Ölçüm tamamlandı ve JSON raporu kaydedildi.'); } }),
      subscribe<string>('measurement-error', value => { if (active) { setMeasuring(false); notify(value, true); } }),
      subscribe('measurement-stopped', () => { if (active) setMeasuring(false); }),
    ];
    return () => { active = false; listeners.forEach(listener => void listener.then(stop => stop())); };
  }, [notify]);

  useEffect(() => {
    if (!desktop || measuring) return;
    let active = true;
    let pending = false;
    const read = async () => {
      if (pending || document.hidden) return;
      pending = true;
      try { const value = await command<ResourceSample>('sample_resources'); if (active) setSample(value); }
      catch (error) { if (active) notify(errorText(error), true); }
      finally { pending = false; }
    };
    void read();
    const timer = setInterval(() => void read(), 2000);
    return () => { active = false; clearInterval(timer); };
  }, [measuring, notify]);

  const start = async () => {
    try { await command('start_measurement', { durationSeconds: 60 }); setSamples([]); setMeasuring(true); }
    catch (error) { notify(errorText(error), true); }
  };
  const elapsed = measuring ? Math.min(60, Math.floor((sample?.elapsedMs ?? 0) / 1000)) : 0;

  return <section className="performance-view">
    <div className="metrics-grid">
      <div className="metric"><div className="metric-label"><HardDrive size={16} />RAM <span>Özel çalışma kümesi</span></div><div className="metric-value">{number(sample?.privateWorkingSetMib)}<span>MiB</span></div><p>Rust + uygulamaya ait WebView2 süreçleri</p></div>
      <div className="metric"><div className="metric-label"><Cpu size={16} />CPU <span>Makine toplamı</span></div><div className="metric-value">{number(sample?.cpuPercent, 2)}<span>%</span></div><p>İki ölçüm arasındaki işlemci süresi</p></div>
      <div className="metric"><div className="metric-label"><Activity size={16} />Süreçler <span>Uygulama ağacı</span></div><div className="metric-value">{sample?.processCount ?? '—'}<span>süreç</span></div><p>Steam ve oyunların tüketimi dahil edilmez</p></div>
    </div>
    {sample?.partial && <div className="message error">Bazı süreçlere erişilemedi. Ölçüm kısmi; toplam tüketim daha yüksek olabilir.</div>}
    <div className="measurement-panel"><div className="measurement-heading"><div><h2>60 saniyelik ölçüm</h2><p>{measuring ? 'Ölçüm sürüyor. Tepsiye geçersen arka plan tüketimini de kaydedebilirsin.' : 'Boşta bırak veya normal kullan. Sonuç yerel bir JSON raporuna kaydedilir.'}</p></div>{measuring ? <button className="button" onClick={() => void command('cancel_measurement').catch(error => notify(errorText(error), true))}><Square size={14} />Durdur</button> : <button className="button primary" onClick={() => void start()} disabled={!desktop}><Play size={15} fill="currentColor" />Ölçümü başlat</button>}</div>
      {measuring && <div className="measurement-progress"><div><LoaderCircle className="spin" size={14} /><span>{elapsed} / 60 saniye</span></div><progress max={60} value={elapsed} aria-label="Ölçüm ilerlemesi" /></div>}
      {samples.length > 1 && <div className="measurement-chart"><span className="muted">RAM · MiB</span><Sparkline values={samples.map(value => value.privateWorkingSetMib ?? value.workingSetMib)} /></div>}
      {report && <div className="report-summary"><span><Clock3 size={15} />Son ölçüm · {report.durationSeconds} sn · {report.build}</span><div><strong>{number(report.averagePrivateWorkingSetMib)} MiB</strong><span>ortalama RAM</span><strong>{number(report.averageCpuPercent, 2)}%</strong><span>ortalama CPU</span><strong>{number(report.peakPrivateWorkingSetMib)} MiB</strong><span>en yüksek RAM</span></div><button className="text-button" onClick={() => void command('open_measurements_folder').catch(error => notify(errorText(error), true))}><FolderOpen size={15} />Rapor klasörünü aç<ArrowUpRight size={13} /></button></div>}
    </div>
    <div className="process-panel"><div className="section-heading"><h2>Süreç ayrıntıları</h2><span>Bu ekran açıkken 2 saniyede bir güncellenir</span></div><table><thead><tr><th>Süreç</th><th>PID</th><th>Özel RAM</th><th>Çalışma kümesi</th><th>Private bytes</th></tr></thead><tbody>{sample?.processes.map(process => <tr key={process.pid}><td>{process.name}</td><td>{process.pid}</td><td>{number(process.privateWorkingSetMib)} MiB</td><td>{number(process.workingSetMib)} MiB</td><td>{number(process.privateBytesMib)} MiB</td></tr>) ?? <tr><td colSpan={5} className="table-empty">{desktop ? 'İlk ölçüm bekleniyor' : 'Masaüstü bağlantısı gerekli'}</td></tr>}</tbody></table></div>
    <p className="performance-note">Özel RAM fiziksel bellekteki uygulamaya özel sayfaları, private bytes ayrılmış özel belleği gösterir. Toplam çalışma kümesi paylaşılan sayfaları tekrar sayabilir. Release sürümünü ölç; performans ekranının yenilenmesi de küçük bir ölçüm yükü oluşturur.</p>
  </section>;
}

function Sparkline({ values }: { values: number[] }) {
  const min = Math.min(...values), max = Math.max(...values), span = Math.max(max - min, 1);
  const points = values.map((value, index) => `${index / Math.max(values.length - 1, 1) * 1000},${70 - (value - min) / span * 56}`).join(' ');
  return <svg viewBox="0 0 1000 80" preserveAspectRatio="none" role="img" aria-label={`RAM ${number(min)} ile ${number(max)} MiB arasında`}><polyline points={points} fill="none" stroke="currentColor" strokeWidth="2" vectorEffect="non-scaling-stroke" /></svg>;
}
