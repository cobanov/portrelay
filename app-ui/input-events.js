// Physical key positions. The receiving desktop chooses the keyboard layout.
const InputEvents = (() => {
  const keys = {
    Digit1:2,Digit2:3,Digit3:4,Digit4:5,Digit5:6,Digit6:7,Digit7:8,Digit8:9,Digit9:10,Digit0:11,
    Minus:12,Equal:13,Backspace:14,Tab:15,KeyQ:16,KeyW:17,KeyE:18,KeyR:19,KeyT:20,KeyY:21,KeyU:22,KeyI:23,KeyO:24,KeyP:25,
    BracketLeft:26,BracketRight:27,Enter:28,ControlLeft:29,KeyA:30,KeyS:31,KeyD:32,KeyF:33,KeyG:34,KeyH:35,KeyJ:36,KeyK:37,KeyL:38,
    Semicolon:39,Quote:40,Backquote:41,ShiftLeft:42,Backslash:43,KeyZ:44,KeyX:45,KeyC:46,KeyV:47,KeyB:48,KeyN:49,KeyM:50,Comma:51,Period:52,Slash:53,
    ShiftRight:54,NumpadMultiply:55,AltLeft:56,Space:57,CapsLock:58,F1:59,F2:60,F3:61,F4:62,F5:63,F6:64,F7:65,F8:66,F9:67,F10:68,
    NumLock:69,ScrollLock:70,Numpad7:71,Numpad8:72,Numpad9:73,NumpadSubtract:74,Numpad4:75,Numpad5:76,Numpad6:77,NumpadAdd:78,Numpad1:79,Numpad2:80,Numpad3:81,Numpad0:82,NumpadDecimal:83,
    IntlBackslash:86,F11:87,F12:88,NumpadEnter:96,ControlRight:97,NumpadDivide:98,AltRight:100,Home:102,ArrowUp:103,PageUp:104,ArrowLeft:105,ArrowRight:106,End:107,ArrowDown:108,PageDown:109,Insert:110,Delete:111,MetaLeft:125,MetaRight:126,ContextMenu:127,
  };
  class Queue {
    constructor() { this.events = []; this.motion = [0, 0]; this.scroll = [0, 0]; }
    push(event) {
      const last = this.events.at(-1);
      if (last?.type === event.type && ["move", "wheel"].includes(event.type)) {
        const bound = event.type === "move" ? 32767 : 16;
        if (Math.abs(last.x + event.x) <= bound && Math.abs(last.y + event.y) <= bound) {
          last.x += event.x; last.y += event.y; return;
        }
      }
      if (this.events.length >= 64) throw new Error("Input queue filled. Control stopped; click to reconnect.");
      this.events.push(event);
    }
    delta(type, x, y, fractions) {
      if (!Number.isFinite(x) || !Number.isFinite(y)) return;
      fractions[0] += x; fractions[1] += y;
      const dx = Math.trunc(fractions[0]), dy = Math.trunc(fractions[1]);
      fractions[0] -= dx; fractions[1] -= dy;
      const bound = type === "move" ? 32767 : 16;
      if (Math.abs(dx) > bound || Math.abs(dy) > bound) throw new Error("Input motion exceeded the limit. Control stopped.");
      if (dx || dy) this.push({ type, x: dx, y: dy });
    }
    move(x, y) { this.delta("move", x, y, this.motion); }
    wheel(x, y, mode) {
      const scale = mode === 1 ? 1 / 3 : mode === 2 ? 3 : 1 / 100;
      this.delta("wheel", x * scale, -y * scale, this.scroll);
    }
    take() { return this.events.splice(0); }
  }
  return { keys, Queue };
})();
if (typeof module !== "undefined") module.exports = InputEvents;
