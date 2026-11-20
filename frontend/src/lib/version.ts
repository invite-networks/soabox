declare const __APP_VERSION__: string;

/** The SOAbox version, taken from package.json at build time. */
export const APP_VERSION: string = typeof __APP_VERSION__ === "string" ? __APP_VERSION__ : "dev";
