function preventIOSMagnifier(): void {
  // prevent magnifier
  function createHandler(
    func: ((event: Event) => void) | null,
    timeout: number
  ): (this: any) => void {
    let timer: number | null = null;
    let pressed: boolean = false;

    return function (this: any): void {
      // this function will be called for every touch start
      if (timer !== null) {
        clearTimeout(timer);
      }

      if (pressed) {
        if (func) {
          func.apply(this, arguments as any);
        }
        clear();
      } else {
        pressed = true;
        timer = setTimeout(clear, timeout || 500) as unknown as number;
      }
    };

    function clear(): void {
      timer = null;
      pressed = false;
    }
  }

  const ignore = createHandler((e: Event): void => {
    e.preventDefault();
  }, 500);


  // TODO: further check if applying to body is too aggressive, maybe apply to map container instead.
  document.body.addEventListener("touchstart", ignore, { passive: false });
  document.body.addEventListener(
    "touchend",
    (e: Event): void => {
      e.preventDefault();
    },
    { passive: false }
  );
}

export function disableMagnifierIfIOS(): void {
  const ua: string = navigator.userAgent;
  const isIOS: boolean = /iPad|iPhone|iPod/.test(ua);
  if (isIOS) {
    preventIOSMagnifier();
  }
}
