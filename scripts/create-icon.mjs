// Original geometric placeholder icon. No artwork from the reference app.
import { writeFileSync } from 'node:fs';
const images = [16, 32, 48, 64].map(size => {
  const stride = Math.ceil(size / 32) * 4;
  const data = Buffer.alloc(40 + size * size * 4 + stride * size);
  data.writeUInt32LE(40); data.writeInt32LE(size, 4); data.writeInt32LE(size * 2, 8);
  data.writeUInt16LE(1, 12); data.writeUInt16LE(32, 14);
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    let coverage = 0;
    for (let sy = 0; sy < 4; sy++) for (let sx = 0; sx < 4; sx++) {
      const u = (x + (sx + .5) / 4) / size, v = (y + (sy + .5) / 4) / size;
      const lid = u >= .2 && u <= .8 && v >= .23 && v <= .30;
      const handle = u >= .39 && u <= .61 && v >= .14 && v <= .24;
      const body = v >= .35 && v <= .84 && u >= .28 + (v-.35)*.07 && u <= .72 - (v-.35)*.07;
      const slots = v >= .43 && v <= .73 && ((u >= .42 && u <= .46) || (u >= .54 && u <= .58));
      if (lid || handle || (body && !slots)) coverage++;
    }
    const at = 40 + ((size - 1 - y) * size + x) * 4;
    data[at] = 250; data[at+1] = 180; data[at+2] = 137; data[at+3] = Math.round(255*coverage/16);
    if (!coverage) data[40+size*size*4+(size-1-y)*stride+(x>>3)] |= 1<<(7-(x&7));
  }
  return { size, data };
});
const header = Buffer.alloc(6 + images.length * 16);
header.writeUInt16LE(1, 2); header.writeUInt16LE(images.length, 4);
let offset = header.length;
images.forEach(({size,data},i) => {
  const at = 6 + i*16;
  header[at] = size; header[at+1] = size;
  header.writeUInt16LE(1,at+4); header.writeUInt16LE(32,at+6);
  header.writeUInt32LE(data.length,at+8); header.writeUInt32LE(offset,at+12); offset += data.length;
});
writeFileSync(new URL('../resources/app.ico', import.meta.url),Buffer.concat([header,...images.map(i=>i.data)]));
