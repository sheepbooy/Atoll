// Mean-luminance test so dark covers can relax the backdrop scrim instead of
// collapsing into a dead black slab.
export function sampleArtworkIsDark(base64: string): Promise<boolean> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      try {
        const size = 24;
        const canvas = document.createElement("canvas");
        canvas.width = size;
        canvas.height = size;
        const ctx = canvas.getContext("2d");
        if (!ctx) {
          resolve(false);
          return;
        }
        ctx.drawImage(img, 0, 0, size, size);
        const { data } = ctx.getImageData(0, 0, size, size);
        let luminance = 0;
        for (let i = 0; i < data.length; i += 4) {
          luminance += 0.2126 * data[i] + 0.7152 * data[i + 1] + 0.0722 * data[i + 2];
        }
        resolve(luminance / (data.length / 4) / 255 < 0.3);
      } catch {
        resolve(false);
      }
    };
    img.onerror = () => resolve(false);
    img.src = `data:image/jpeg;base64,${base64}`;
  });
}
