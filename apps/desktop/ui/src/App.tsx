import { useCallback, useEffect, useRef, useState } from 'react';
import { Activity, ArrowDownToLine, ArrowUpRight, Check, ChevronRight, FolderPlus, Gamepad2, HardDrive, LayoutGrid, Library, LoaderCircle, Play, RefreshCw, Search, Star, X } from 'lucide-react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useVirtualizer } from '@tanstack/react-virtual';
import { command, desktop, errorText, subscribe } from './api';
import type { Game, GamePage, LibraryStatus, ScanReport } from './bindings';
import { Performance } from './Performance';

type View = 'library' | 'favorites' | 'performance' | 'sources';
const emptyStatus: LibraryStatus = { sources: [], total: 0, installed: 0, favorites: 0, roots: [], warnings: [], lastScanUnixMs: null, lastScanDurationMs: null };
const formatSize = (bytes: number) => bytes ? `${(bytes / 1_073_741_824).toLocaleString('tr-TR', { maximumFractionDigits: 1 })} GB` : 'Steam';

export function App() {
  const [view, setView] = useState<View>('library');
  const [status, setStatus] = useState(emptyStatus);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [revision, setRevision] = useState(0);
  const [scanning, setScanning] = useState(false);
  const [notice, setNotice] = useState<{ text: string; error: boolean } | null>(null);
  const [selected, setSelected] = useState<Game | null>(null);
  const [launching, setLaunching] = useState<string | null>(null);
  const search = useRef<HTMLInputElement>(null);

  const notify = useCallback((text: string, error = false) => setNotice({ text, error }), []);
  const refresh = useCallback(async () => {
    try { setStatus(await command<LibraryStatus>('library_status')); setRevision(value => value + 1); }
    catch (error) { notify(errorText(error), true); }
  }, [notify]);

  const scan = useCallback(async () => {
    setScanning(true);
    try {
      const report = await command<ScanReport>('scan_steam');
      await refresh();
      if (report.cancelled) notify('Tarama durduruldu. Önceki kayıtlar korundu.');
      else notify(report.roots.length ? `${report.found} oyun bulundu · ${Math.round(report.durationMs)} ms` : 'Steam bulunamadı. Bir Steam veya SteamLibrary klasörü ekleyebilirsin.');
    } catch (error) { notify(errorText(error), true); }
    finally { setScanning(false); }
  }, [notify, refresh]);

  useEffect(() => {
    if (!desktop) return;
    let active = true;
    const listener = subscribe<ScanReport>('library-changed', () => { if (active) void refresh(); });
    void refresh().then(() => { if (active) void scan(); });
    void command('ui_ready');
    return () => { active = false; void listener.then(stop => stop()); };
  }, [refresh, scan]);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.ctrlKey && event.key.toLowerCase() === 'k') { event.preventDefault(); setView('library'); requestAnimationFrame(() => search.current?.focus()); }
      if (event.key === 'Escape') setSelected(null);
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  const addFolder = async () => {
    setScanning(true);
    try {
      const report = await command<ScanReport | null>('add_steam_folder');
      if (report) { await refresh(); notify(`${report.found} oyun bulundu`); }
    } catch (error) { notify(errorText(error), true); }
    finally { setScanning(false); }
  };

  const favorite = async (game: Game) => {
    try {
      await command('set_favorite', { id: game.id, favorite: !game.favorite });
      if (selected?.id === game.id) setSelected({ ...game, favorite: !game.favorite });
      await refresh();
    } catch (error) { notify(errorText(error), true); }
  };

  const launch = async (game: Game) => {
    setLaunching(game.id);
    try { notify(await command<string>('launch_game', { id: game.id })); }
    catch (error) { notify(errorText(error), true); }
    finally { setLaunching(null); }
  };

  const go = (next: View) => { setView(next); setSelected(null); };
  const title = view === 'favorites' ? 'Favorilerin' : view === 'performance' ? 'Performans' : view === 'sources' ? 'Oyun kaynakları' : 'Kütüphanen';

  return <div className="shell">
    <aside className="sidebar">
      <div className="brand"><span className="brand-mark"><Gamepad2 size={22} /></span><div>Game Library<span>MASAÜSTÜ</span></div></div>
      <div className="nav-label">KİŞİSEL ALAN</div>
      <nav aria-label="Ana navigasyon">
        <button className={view === 'library' ? 'nav-item active' : 'nav-item'} onClick={() => go('library')}><Library size={18} />Kütüphane<span>{status.total}</span></button>
        <button className={view === 'favorites' ? 'nav-item active' : 'nav-item'} onClick={() => go('favorites')}><Star size={18} />Favoriler<span>{status.favorites}</span></button>
        <button className={view === 'performance' ? 'nav-item active' : 'nav-item'} onClick={() => go('performance')}><Activity size={18} />Performans</button>
      </nav>
      <div className="nav-label sources-label">KAYNAKLAR</div>
      <button className={view === 'sources' ? 'nav-item active' : 'nav-item'} onClick={() => go('sources')}><span className="steam-mark">S</span>Steam<span>{status.installed}</span></button>
      <button className="add-source" disabled={scanning || !desktop} onClick={() => void addFolder()}><FolderPlus size={16} />Klasör ekle</button>
      <div className="sidebar-bottom"><span className="status-dot" />Yerel kütüphane<span className="version">v0.1.0</span></div>
    </aside>

    <div className="workspace">
      <header className="topbar">
        <div className="breadcrumb">Kişisel alan<ChevronRight size={14} /><span>{title}</span></div>
        <button className="button quiet tray-button" onClick={() => void command('minimize_to_tray').catch(error => notify(errorText(error), true))} disabled={!desktop}><ArrowDownToLine size={16} />Tepsiye al</button>
      </header>
      <main>
        <div className="page-heading"><div><div className="eyebrow">{view === 'performance' ? 'KAYNAK KULLANIMI' : view === 'sources' ? 'KÜTÜPHANE BAĞLANTILARI' : 'OYUNLARIN, TEK YERDE'}</div><h1>{title}</h1><p>{view === 'performance' ? 'Uygulamanın gerçek RAM ve CPU tüketimini ölç.' : view === 'sources' ? 'Steam kurulumunu ve ek oyun kütüphanelerini yönet.' : 'Yüklü oyunlarını bul, düzenle ve kaldığın yerden devam et.'}</p></div>
          {(view === 'library' || view === 'favorites' || view === 'sources') && <button className="button" disabled={scanning || !desktop} onClick={() => void scan()}>{scanning ? <LoaderCircle size={16} className="spin" /> : <RefreshCw size={16} />}{scanning ? 'Taranıyor' : 'Yeniden tara'}</button>}
        </div>

        {!desktop && <div className="message error">Masaüstü bağlantısı yok. Oyun tarama ve ölçüm için Game Library uygulamasını çalıştır.</div>}
        {notice && <div className={`message ${notice.error ? 'error' : ''}`} role={notice.error ? 'alert' : 'status'}>{notice.error ? <X size={16} /> : <Check size={16} />}<span>{notice.text}</span><button className="icon-button" aria-label="Bildirimi kapat" onClick={() => setNotice(null)}><X size={15} /></button></div>}
        {scanning && <div className="scan-line"><span>Steam manifestleri okunuyor</span><button className="text-button" onClick={() => void command('cancel_scan')}>Durdur</button></div>}

        {view === 'performance' ? <Performance notify={notify} /> : view === 'sources' ? <Sources status={status} addFolder={() => void addFolder()} disabled={scanning || !desktop} /> : <>
          <div className="library-toolbar">
            <div className="filter-tabs" aria-label="Kurulum filtresi">
              <button aria-pressed={filter === 'all'} onClick={() => setFilter('all')}>Tümü<span>{view === 'favorites' ? status.favorites : status.total}</span></button>
              <button aria-pressed={filter === 'installed'} onClick={() => setFilter('installed')}>Kurulu</button>
            </div>
            <label className="search-field"><Search size={16} /><input ref={search} value={query} onChange={event => setQuery(event.target.value)} placeholder="Oyun ara" aria-label="Oyun ara" /><kbd>Ctrl K</kbd></label>
            <span className="grid-indicator" aria-label="Kapak görünümü"><LayoutGrid size={18} /></span>
          </div>
          <GameGrid query={query} filter={view === 'favorites' ? filter === 'installed' ? 'favorites-installed' : 'favorites' : filter} revision={revision} favorite={favorite} launch={launch} launching={launching} select={setSelected} notify={notify} addFolder={() => void addFolder()} scanning={scanning} />
        </>}
      </main>
      <footer className="footer"><span><HardDrive size={13} />{status.roots.length} kütüphane · {status.installed} kurulu oyun</span><span>{status.lastScanUnixMs ? `Son tarama ${new Date(status.lastScanUnixMs).toLocaleTimeString('tr-TR', { hour: '2-digit', minute: '2-digit' })}` : 'Henüz taranmadı'}{status.lastScanDurationMs !== null && ` · ${Math.round(status.lastScanDurationMs)} ms`}</span></footer>
    </div>
    {selected && <GameDetails game={selected} close={() => setSelected(null)} favorite={() => void favorite(selected)} launch={() => void launch(selected)} launching={launching === selected.id} />}
  </div>;
}

function GameGrid({ query, filter, revision, favorite, launch, launching, select, notify, addFolder, scanning }: {
  query: string; filter: string; revision: number; favorite: (game: Game) => Promise<void>; launch: (game: Game) => Promise<void>; launching: string | null; select: (game: Game) => void; notify: (text: string, error?: boolean) => void; addFolder: () => void; scanning: boolean;
}) {
  const [games, setGames] = useState<Game[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [columns, setColumns] = useState(4);
  const scroller = useRef<HTMLDivElement>(null);
  const epoch = useRef(0);
  const fetching = useRef(false);
  const loaded = useRef(0);
  const [debounced, setDebounced] = useState(query);
  useEffect(() => { const timer = setTimeout(() => setDebounced(query), 120); return () => clearTimeout(timer); }, [query]);
  useEffect(() => {
    const element = scroller.current;
    if (!element) return;
    const observer = new ResizeObserver(entries => setColumns(Math.max(2, Math.floor(entries[0].contentRect.width / 190))));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const version = ++epoch.current;
    loaded.current = 0;
    fetching.current = true;
    setLoading(true);
    setGames([]);
    setTotal(0);
    scroller.current?.scrollTo({ top: 0 });
    if (!desktop) { setLoading(false); fetching.current = false; return; }
    void command<GamePage>('list_games', { query: debounced, filter, offset: 0, limit: 200 }).then(page => {
      if (version !== epoch.current) return;
      setGames(page.games); setTotal(page.total); loaded.current = page.games.length;
    }).catch(error => { if (version === epoch.current) notify(errorText(error), true); }).finally(() => { if (version === epoch.current) { fetching.current = false; setLoading(false); } });
  }, [debounced, filter, revision, notify]);

  const visibleGames = games;
  const rows = Math.ceil(visibleGames.length / columns);
  const virtual = useVirtualizer({ count: rows + (loaded.current < total ? 1 : 0), getScrollElement: () => scroller.current, estimateSize: () => 310, overscan: 2 });
  const items = virtual.getVirtualItems();
  const lastRow = items.at(-1)?.index ?? 0;

  useEffect(() => {
    if (!desktop || loading || fetching.current || loaded.current >= total || lastRow < rows - 3) return;
    const version = epoch.current;
    fetching.current = true;
    void command<GamePage>('list_games', { query: debounced, filter, offset: loaded.current, limit: 200 }).then(page => {
      if (version !== epoch.current) return;
      loaded.current += page.games.length;
      setGames(previous => [...previous, ...page.games]);
    }).catch(error => notify(errorText(error), true)).finally(() => { if (version === epoch.current) fetching.current = false; });
  }, [lastRow, rows, total, loading, debounced, filter, notify]);

  return <div ref={scroller} className="game-scroller">
    {loading ? <div className="empty-state"><LoaderCircle className="spin" size={25} /><p>Kütüphane yükleniyor</p></div> : !visibleGames.length && total === 0 ? <div className="empty-state"><div className="empty-icon"><Gamepad2 size={34} /></div><h2>{debounced ? 'Oyun bulunamadı' : filter === 'favorites' ? 'Favorilerin burada görünecek' : 'Oyunlarına yer açtık'}</h2><p>{debounced ? 'Farklı bir oyun adıyla tekrar ara.' : filter === 'favorites' ? 'Bir oyunun yıldızına dokunarak favorilerine ekle.' : 'Steam oyunlarını otomatik bulacağız. Farklı bir kurulumun varsa kütüphane klasörünü seç.'}</p>{!debounced && filter !== 'favorites' && <button className="button primary" disabled={!desktop || scanning} onClick={addFolder}><FolderPlus size={17} />Steam klasörü ekle</button>}<span className="empty-note">Hesap bağlantısı veya internet gerekmez</span></div> : <div className="virtual-grid" style={{ height: virtual.getTotalSize() }}>
      {items.map(row => <div key={row.key} data-index={row.index} ref={virtual.measureElement} className="game-row" style={{ transform: `translateY(${row.start}px)`, gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}>
        {visibleGames.slice(row.index * columns, (row.index + 1) * columns).map(game => <GameCard key={game.id} game={game} favorite={() => void favorite(game)} launch={() => void launch(game)} select={() => select(game)} launching={launching === game.id} />)}
        {row.index === rows && loaded.current < total && <span className="loading-more">Diğer oyunlar yükleniyor…</span>}
      </div>)}
    </div>}
  </div>;
}

function GameCard({ game, favorite, launch, select, launching }: { game: Game; favorite: () => void; launch: () => void; select: () => void; launching: boolean }) {
  const [failedImage, setFailedImage] = useState(false);
  return <article className="game-card">
    <button className="cover" onClick={select} aria-label={`${game.name} ayrıntıları`}>
      {game.artworkPath && !failedImage ? <img src={convertFileSrc(game.artworkPath)} alt="" loading="lazy" decoding="async" onError={() => setFailedImage(true)} /> : <div className="cover-fallback"><Gamepad2 size={34} /><span>{game.name}</span><span className="fallback-source">STEAM</span></div>}
      <span className={`install-badge ${game.installed ? '' : 'missing'}`}>{game.installed ? 'Kurulu' : 'Eksik'}</span>
    </button>
    <button className={`favorite-button ${game.favorite ? 'is-favorite' : ''}`} aria-label={`${game.name} ${game.favorite ? 'favorilerden çıkar' : 'favorilere ekle'}`} aria-pressed={game.favorite} onClick={favorite}><Star size={16} fill={game.favorite ? 'currentColor' : 'none'} /></button>
    <div className="game-info"><button className="game-name" onClick={select}>{game.name}</button><div className="game-meta"><span>{formatSize(game.sizeBytes)}</span><button className="play-button" disabled={!game.installed || launching} onClick={launch} aria-label={`${game.name} başlat`}>{launching ? <LoaderCircle size={13} className="spin" /> : <Play size={13} fill="currentColor" />}Oyna</button></div></div>
  </article>;
}

function Sources({ status, addFolder, disabled }: { status: LibraryStatus; addFolder: () => void; disabled: boolean }) {
  return <section className="sources-view"><div className="source-heading"><span className="source-logo">S</span><div><h2>Steam</h2><p>Registry ve yerel oyun manifestleri</p></div><span className="connection-badge">{status.roots.length ? 'Algılandı' : 'Kurulum bulunamadı'}</span></div><div className="source-body"><h3>Kütüphane klasörleri</h3>{status.roots.length ? status.roots.map(root => <div className="root-path" key={root}><HardDrive size={17} /><span>{root.replace(/^\\\\\?\\/, '')}</span><Check size={16} /></div>) : <p className="muted">Bu bilgisayarda Steam klasörü bulunamadı. Steam kurulumunu veya ek bir SteamLibrary klasörünü seçebilirsin.</p>}<button className="button" disabled={disabled} onClick={addFolder}><FolderPlus size={16} />Klasör ekle</button></div>{status.warnings.length > 0 && <div className="source-body"><h3>Tarama uyarıları</h3><ul className="warnings">{status.warnings.map((warning, index) => <li key={index}>{warning}</li>)}</ul></div>}<p className="source-note">Oyunlar Steam üzerinden başlatılır. Steam hesabına giriş gerekirse Steam kendi ekranını açar.</p></section>;
}

function GameDetails({ game, close, favorite, launch, launching }: { game: Game; close: () => void; favorite: () => void; launch: () => void; launching: boolean }) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { dialog.current?.showModal(); }, []);
  return <dialog ref={dialog} className="game-dialog" onCancel={close} onClick={event => { if (event.target === dialog.current) close(); }} aria-labelledby="game-title"><div className="dialog-top"><span className="eyebrow">STEAM KÜTÜPHANESİ</span><button className="icon-button" aria-label="Ayrıntıları kapat" onClick={close}><X size={19} /></button></div><h2 id="game-title">{game.name}</h2><div className="detail-badges"><span>{game.installed ? 'Kurulu' : 'Kurulum erişilemiyor'}</span><span>{formatSize(game.sizeBytes)}</span><span>AppID {game.appId}</span></div><div className="detail-path"><span>Kurulum</span><p>{game.installPath.replace(/^\\\\\?\\/, '')}</p></div><div className="dialog-actions"><button className="button primary" disabled={!game.installed || launching} onClick={launch}>{launching ? <LoaderCircle size={16} className="spin" /> : <Play size={16} fill="currentColor" />}Steam üzerinden başlat<ArrowUpRight size={16} /></button><button className="button" onClick={favorite}><Star size={16} fill={game.favorite ? 'currentColor' : 'none'} />{game.favorite ? 'Favorilerde' : 'Favorilere ekle'}</button></div></dialog>;
}
