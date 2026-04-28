#!/usr/bin/env node
// Renders the READZIP banner PNG.
// Source: oh-my-logo "READZIP" grad-blue --filled, drawn with @napi-rs/canvas
// using a system monospace font that supports Unicode box-drawing glyphs.
//
//   npm i -D @napi-rs/canvas
//   node assets/brand/build.js assets/brand/readzip.png

const fs = require("fs");
const path = require("path");
const { createCanvas, GlobalFonts } = require("@napi-rs/canvas");

const fontCandidates = [
  "/System/Library/Fonts/Menlo.ttc",
  "/System/Library/Fonts/SFNSMono.ttf",
  "/Library/Fonts/Menlo.ttf",
  "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
  "/usr/share/fonts/truetype/liberation/LiberationMono-Bold.ttf",
];
let fontFamily = "monospace";
for (const f of fontCandidates) {
  if (fs.existsSync(f)) {
    GlobalFonts.registerFromPath(f, "Mono");
    fontFamily = "Mono";
    break;
  }
}

const lines = [
  "██████╗ ███████╗ █████╗ ██████╗ ███████╗██╗██████╗ ",
  "██╔══██╗██╔════╝██╔══██╗██╔══██╗╚══███╔╝██║██╔══██╗",
  "██████╔╝█████╗  ███████║██║  ██║  ███╔╝ ██║██████╔╝",
  "██╔══██╗██╔══╝  ██╔══██║██║  ██║ ███╔╝  ██║██╔═══╝ ",
  "██║  ██║███████╗██║  ██║██████╔╝███████╗██║██║     ",
  "╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝╚═════╝ ╚══════╝╚═╝╚═╝     ",
];

const fontSize = 36;
const lineH = 38;
const padX = 80;
const padTop = 100;
const tagH = 80;

const measure = createCanvas(10, 10).getContext("2d");
measure.font = `bold ${fontSize}px ${fontFamily}`;
const maxLineWidth = Math.max(...lines.map(l => measure.measureText(l).width));

const W = Math.ceil(maxLineWidth + padX * 2);
const H = padTop + lines.length * lineH + tagH;

const canvas = createCanvas(W, H);
const ctx = canvas.getContext("2d");

const r = 24;
const bg = ctx.createLinearGradient(0, 0, 0, H);
bg.addColorStop(0, "#0b1220");
bg.addColorStop(1, "#060912");
ctx.fillStyle = bg;
ctx.beginPath();
ctx.moveTo(r, 0);
ctx.lineTo(W - r, 0);
ctx.quadraticCurveTo(W, 0, W, r);
ctx.lineTo(W, H - r);
ctx.quadraticCurveTo(W, H, W - r, H);
ctx.lineTo(r, H);
ctx.quadraticCurveTo(0, H, 0, H - r);
ctx.lineTo(0, r);
ctx.quadraticCurveTo(0, 0, r, 0);
ctx.closePath();
ctx.fill();

for (let i = 0; i < 3; i++) {
  ctx.beginPath();
  ctx.arc(padX + i * 22, 50, 6, 0, Math.PI * 2);
  ctx.fillStyle = "#1f2a3d";
  ctx.fill();
}

const grad = ctx.createLinearGradient(0, padTop, 0, padTop + lines.length * lineH);
grad.addColorStop(0, "#4ea8ff");
grad.addColorStop(1, "#7f88ff");
ctx.fillStyle = grad;
ctx.font = `bold ${fontSize}px ${fontFamily}`;
ctx.textBaseline = "top";
for (let i = 0; i < lines.length; i++) {
  ctx.fillText(lines[i], padX, padTop + i * lineH);
}

ctx.fillStyle = "#8fd3ff";
ctx.font = `20px ${fontFamily}`;
const tagY = padTop + lines.length * lineH + 30;
ctx.fillText("structural reads for coding agents", padX, tagY);

ctx.fillStyle = "#4ea8ff";
ctx.globalAlpha = 0.7;
const stat = "~81% fewer tokens";
const statW = ctx.measureText(stat).width;
ctx.fillText(stat, W - padX - statW, tagY);
ctx.globalAlpha = 1;

const out = process.argv[2] || path.join(__dirname, "readzip.png");
fs.writeFileSync(out, canvas.toBuffer("image/png"));
console.log("wrote", out, `${W}x${H}`);
