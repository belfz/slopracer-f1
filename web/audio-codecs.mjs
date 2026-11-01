if (typeof globalThis.TextDecoder === "undefined") {
    globalThis.TextDecoder = class {
        decode(input = new Uint8Array()) {
            const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
            let text = "";
            for (let index = 0; index < bytes.length;) {
                const first = bytes[index++];
                if (first < 0x80) {
                    text += String.fromCharCode(first);
                    continue;
                }
                const size = first >= 0xc2 && first <= 0xdf ? 2 : first >= 0xe0 && first <= 0xef ? 3 : first >= 0xf0 && first <= 0xf4 ? 4 : 0;
                let point = first & (0x7f >> size);
                let valid = size > 0 && index + size - 1 <= bytes.length;
                for (let offset = 0; valid && offset < size - 1; offset++) {
                    const next = bytes[index + offset];
                    valid = next >= 0x80 && next <= 0xbf;
                    point = (point << 6) | (next & 0x3f);
                }
                const minimum = size === 2 ? 0x80 : size === 3 ? 0x800 : 0x10000;
                if (!valid || point < minimum || point > 0x10ffff || (point >= 0xd800 && point <= 0xdfff)) {
                    text += "\ufffd";
                    continue;
                }
                index += size - 1;
                text += String.fromCodePoint(point);
            }
            return text;
        }
    };
}

if (typeof globalThis.TextEncoder === "undefined") {
    globalThis.TextEncoder = class {
        encode(text = "") {
            const buffer = new Uint8Array(text.length * 3);
            const { written } = this.encodeInto(text, buffer);
            return buffer.slice(0, written);
        }

        encodeInto(text, buffer) {
            let read = 0;
            let written = 0;
            for (const character of text) {
                let point = character.codePointAt(0);
                if (point >= 0xd800 && point <= 0xdfff) point = 0xfffd;
                const size = point < 0x80 ? 1 : point < 0x800 ? 2 : point < 0x10000 ? 3 : 4;
                if (written + size > buffer.length) break;
                if (size === 1) buffer[written++] = point;
                else {
                    buffer[written++] = (size === 2 ? 0xc0 : size === 3 ? 0xe0 : 0xf0) | (point >> (6 * (size - 1)));
                    for (let shift = (size - 2) * 6; shift >= 0; shift -= 6) buffer[written++] = 0x80 | ((point >> shift) & 0x3f);
                }
                read += character.length;
            }
            return { read, written };
        }
    };
}
