export interface Limits {
  cpu?: number; mem?: number; time?: number; output?: number; file?: number;
  written?: number; net?: number; threads?: number; fds?: number; files?: number;
}
export interface Grants {
  /** Default true: the program gets only what is granted below. */
  sandbox?: boolean;
  allow?: Array<"read" | "write" | "net" | "native">;
  allowPy?: string[];
  allowNet?: string[];
  allowRead?: string[];
  allowWrite?: string[];
  limits?: Limits;
  /** Seconds for build and run together (default 20, at most 600). */
  timeout?: number;
}
export interface SessionOptions extends Grants {
  /** Where brevi mcp starts. */
  cwd?: string;
  /** The brevi executable (default: BREVI_BIN, else the release binary for this platform). */
  brevi?: string;
}
export interface Case { stdin?: string; expect?: string }
export interface RunOptions extends Grants { stdin?: string; args?: string[]; cases?: Case[] }
export interface VerifyOptions { inputs?: string[]; args?: string[]; once?: boolean; limits?: Limits; timeout?: number }
export interface Certificate { verify: string; sha256?: string; path?: string; speedup?: number; [k: string]: unknown }

export declare class Result {
  text: string;
  ok: boolean;
  stdout: string;
  stderr: string;
  exitCode: number | null;
  path: "typed" | "dynamic" | "pybridge" | "cpython" | null;
  ms: number | null;
  timedOut: boolean;
  certificate: Certificate | null;
  readonly limit: string | null;
  forModel(cap?: number): string;
}
export declare class BreviError extends Error {}
export declare class Session {
  constructor(defaults?: SessionOptions);
  call(tool: string, args: Record<string, unknown>): Promise<Result>;
  run(code: string, opts?: RunOptions): Promise<Result>;
  check(code: string, opts?: Grants & { fast?: boolean }): Promise<Result>;
  verify(code: string, opts?: VerifyOptions): Promise<Result>;
  handle(input: { code: string; stdin?: string }): Promise<string>;
  close(): void;
}
export declare const TOOL: {
  name: "run_python";
  description: string;
  input_schema: Record<string, unknown>;
};
export declare function run(code: string, opts?: RunOptions): Promise<Result>;
export declare function check(code: string, opts?: Grants & { fast?: boolean }): Promise<Result>;
export declare function verify(code: string, opts?: VerifyOptions): Promise<Result>;
export declare function handle(input: { code: string; stdin?: string }): Promise<string>;
export declare function binary(): string;
