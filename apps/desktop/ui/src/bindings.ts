export type Source = "steam" | "epic" | "riot" | "manual";

export type LaunchTarget = { "kind": "steam", appId: number, } | { "kind": "epic", namespaceId: string, catalogId: string, appName: string, executable: string, } | { "kind": "riot", clientPath: string, product: string, patchline: string, executable: string, } | { "kind": "manual", executable: string, args: Array<string>, workingDir: string, };

export type SourceStatus = { source: Source, total: number, installed: number, roots: Array<string>, };

export type Game = { id: string, appId: number, source: Source, sourceId: string, launch: LaunchTarget, name: string, libraryPath: string, installPath: string, installed: boolean, favorite: boolean, sizeBytes: number, artworkPath: string | null, };

export type GamePage = { games: Array<Game>, total: number, };

export type LibraryStatus = { sources: Array<SourceStatus>, total: number, installed: number, favorites: number, roots: Array<string>, warnings: Array<string>, lastScanUnixMs: number | null, lastScanDurationMs: number | null, };

export type ScanReport = { roots: Array<string>, found: number, warnings: Array<string>, durationMs: number, cancelled: boolean, };

export type ProcessSample = { pid: number, name: string, workingSetMib: number, privateWorkingSetMib: number | null, privateBytesMib: number, cpuTimeMs: number, creationTime: string, };

export type ResourceSample = { elapsedMs: number, workingSetMib: number, privateWorkingSetMib: number | null, privateBytesMib: number, cpuPercent: number | null, processCount: number, partial: boolean, processes: Array<ProcessSample>, };

export type MeasurementReport = { durationSeconds: number, measuredDurationSeconds: number, maxSampleGapMs: number, interrupted: boolean, sampleCount: number, averageCpuPercent: number | null, averagePrivateWorkingSetMib: number | null, peakPrivateWorkingSetMib: number | null, averageWorkingSetMib: number, averagePrivateBytesMib: number, partial: boolean, logicalProcessors: number, build: string, savedPath: string, samples: Array<ResourceSample>, };
