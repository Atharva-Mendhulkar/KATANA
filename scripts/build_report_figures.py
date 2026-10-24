#!/usr/bin/env python3
import os
import re
import subprocess
from PIL import Image, ImageDraw, ImageFont

ASSETS_DIR = "/home/topfloorboss/Desktop/KATANA/report_assets"
os.makedirs(ASSETS_DIR, exist_ok=True)
FONT_PATH = "/usr/share/fonts/TTF/DejaVuSansMono.ttf"

ANSI_COLOR_MAP = {
    '30': (40, 44, 52),      # black
    '31': (235, 75, 75),     # red (blade)
    '32': (95, 215, 120),    # green
    '33': (240, 200, 80),    # yellow
    '34': (90, 150, 245),    # blue
    '35': (200, 120, 240),   # magenta
    '36': (80, 210, 230),    # cyan
    '37': (230, 235, 245),   # white
    '90': (120, 130, 145),   # bright black / gray
    '91': (255, 95, 95),     # bright red
    '92': (120, 240, 140),   # bright green
    '93': (255, 220, 90),    # bright yellow
    '94': (120, 180, 255),   # bright blue
    '95': (225, 145, 255),   # bright magenta
    '96': (110, 235, 255),   # bright cyan
    '97': (255, 255, 255),   # bright white
}

def parse_ansi_line(line):
    tokens = []
    current_color = (220, 225, 235)
    pattern = re.compile(r'(\x1b\[[0-9;]*m)')
    parts = pattern.split(line)
    for part in parts:
        if not part:
            continue
        if part.startswith('\x1b['):
            code = part[2:-1]
            if code == '' or code == '0':
                current_color = (220, 225, 235)
            else:
                subcodes = code.split(';')
                for sc in subcodes:
                    if sc in ANSI_COLOR_MAP:
                        current_color = ANSI_COLOR_MAP[sc]
        else:
            tokens.append((part, current_color))
    return tokens

def render_terminal(cmd, output_text, output_image_path):
    font_size = 14
    font = ImageFont.truetype(FONT_PATH, font_size)
    bold_font = ImageFont.truetype(FONT_PATH, font_size)

    lines = output_text.splitlines()
    if len(lines) > 45:
        lines = lines[:45] + ["... [truncated for display] ..."]

    # Calculate dimensions
    char_w = 8.5
    line_h = 20
    max_len = max((len(re.sub(r'\x1b\[[0-9;]*m', '', l)) for l in lines), default=80)
    max_len = max(max_len, len(cmd) + 6)
    
    padding_x = 24
    header_h = 36
    padding_y = 16
    
    img_w = int(max(750, max_len * char_w + padding_x * 2))
    img_h = int(header_h + padding_y + (len(lines) + 2) * line_h + padding_y)

    img = Image.new("RGBA", (img_w, img_h), (18, 22, 30, 255))
    draw = ImageDraw.Draw(img)

    # Header bar
    draw.rectangle([(0, 0), (img_w, header_h)], fill=(28, 34, 46, 255))
    draw.line([(0, header_h), (img_w, header_h)], fill=(45, 52, 68, 255), width=1)

    # Window dots
    draw.ellipse([(14, 12), (24, 22)], fill=(255, 95, 86, 255))
    draw.ellipse([(32, 12), (42, 22)], fill=(255, 189, 46, 255))
    draw.ellipse([(50, 12), (60, 22)], fill=(39, 201, 63, 255))

    # Title
    title_text = "katana terminal — " + cmd
    draw.text((75, 10), title_text, font=font, fill=(140, 150, 170, 255))

    # Shell prompt line
    y = header_h + padding_y
    draw.text((padding_x, y), "$ ", font=bold_font, fill=(95, 215, 120, 255))
    draw.text((padding_x + 18, y), cmd, font=bold_font, fill=(255, 255, 255, 255))
    y += line_h + 4

    # Output lines
    for line in lines:
        x = padding_x
        tokens = parse_ansi_line(line)
        for text, color in tokens:
            draw.text((x, y), text, font=font, fill=color)
            bbox = font.getbbox(text)
            text_width = bbox[2] - bbox[0]
            x += text_width
        y += line_h

    # Add 1px subtle outer border
    draw.rectangle([(0, 0), (img_w - 1, img_h - 1)], outline=(55, 65, 85, 255), width=1)
    
    img.save(output_image_path, "PNG")
    print(f"Generated: {output_image_path} ({img_w}x{img_h})")

def capture_command(cmd_args, display_cmd, out_name):
    env = os.environ.copy()
    env["PATH"] = os.path.expanduser("~/.cargo/bin") + ":" + env["PATH"]
    res = subprocess.run(cmd_args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, env=env, text=True)
    out_path = os.path.join(ASSETS_DIR, out_name)
    render_terminal(display_cmd, res.stdout, out_path)

def capture_browser_screenshot(html_path, out_name):
    out_path = os.path.join(ASSETS_DIR, out_name)
    subprocess.run([
        "firefox", "--headless", "--no-remote", "--profile", "/tmp/ff_prof",
        "--screenshot", out_path, "--window-size=1200,800", f"file://{html_path}"
    ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    print(f"Captured browser screenshot: {out_path}")

def render_system_architecture_diagram():
    # Render clear system architecture flow diagram
    w, h = 1000, 520
    img = Image.new("RGB", (w, h), (255, 255, 255))
    draw = ImageDraw.Draw(img)
    font_title = ImageFont.truetype(FONT_PATH, 16)
    font_box = ImageFont.truetype(FONT_PATH, 13)
    font_desc = ImageFont.truetype(FONT_PATH, 11)

    # Boxes:
    boxes = [
        # (x1, y1, x2, y2, title, [bullets], fill, border)
        (40, 50, 240, 210, "Target Process\n(Linux Userspace)", ["Thread T4217 (Leader)", "Worker Threads", "Off-CPU Stalls", "Syscall Invocations"], (245, 247, 250), (60, 90, 140)),
        (280, 50, 480, 210, "Kernel Tracepoints\n(Linux Subsystems)", ["sched_switch (Off-CPU)", "sched_waking (Waker)", "sys_futex (Mutex ops)", "block_rq_* (NVMe/Disk)"], (240, 248, 255), (40, 110, 180)),
        (520, 50, 720, 210, "eBPF In-Kernel Layer\n(katana.bpf.c)", ["Target Filter Hash Map", "Dynamic Expansion (<=8)", "Shared Ringbuf (8 MiB)", "Per-CPU Sequence Ledger"], (248, 245, 255), (120, 70, 180)),
        (760, 50, 960, 210, "Ring Buffer Poller\n(Two-Thread Pipeline)", ["Lockless BPF Drain", "Microsecond Timestamps", "Per-CPU Gap Detection", "Loss Intervals Ledger"], (255, 250, 245), (200, 100, 40)),
        
        (160, 290, 380, 460, "Analysis Core\n(src/events.rs, lib.rs)", ["Total Total Ordering", "Wait-Interval Tracker", "Rule FW-1 (Futex Wake)", "Rule BIO-1/2 (Block I/O)"], (245, 255, 245), (40, 150, 70)),
        (420, 290, 640, 460, "Causal Graph Engine\n(src/graph.rs, evidence.rs)", ["Backward Walk (<=8 hops)", "Cycle / Deadlock Check", "Lexicographic Ranker", "Monotonic Evidence Clamp"], (255, 252, 240), (180, 140, 30)),
        (680, 290, 900, 460, "Presentation Tier\n(CLI, JSON & HTML)", ["Anti-Inflation Verb Table", "JSON API (schema v1)", "Static B&W Visualizer", "Continuous Watch Daemon"], (250, 245, 250), (150, 50, 120)),
    ]

    for x1, y1, x2, y2, title, items, fill, border in boxes:
        draw.rounded_rectangle([(x1, y1), (x2, y2)], radius=8, fill=fill, outline=border, width=2)
        # title
        t_lines = title.splitlines()
        ty = y1 + 10
        for tl in t_lines:
            draw.text((x1 + 14, ty), tl, font=font_title, fill=border)
            ty += 18
        draw.line([(x1 + 10, ty + 2), (x2 - 10, ty + 2)], fill=border, width=1)
        ty += 10
        for it in items:
            draw.text((x1 + 14, ty), "• " + it, font=font_desc, fill=(40, 45, 55))
            ty += 18

    # Draw connection arrows
    arrows = [
        ((240, 130), (280, 130)),
        ((480, 130), (520, 130)),
        ((720, 130), (760, 130)),
        ((860, 210), (860, 250), (270, 250), (270, 290)),
        ((380, 375), (420, 375)),
        ((640, 375), (680, 375)),
    ]

    for arr in arrows:
        if len(arr) == 2:
            p1, p2 = arr
            draw.line([p1, p2], fill=(80, 90, 110), width=2)
            draw.polygon([(p2[0], p2[1]), (p2[0] - 6, p2[1] - 4), (p2[0] - 6, p2[1] + 4)], fill=(80, 90, 110))
        elif len(arr) == 4:
            p1, p2, p3, p4 = arr
            draw.line([p1, p2, p3, p4], fill=(80, 90, 110), width=2)
            draw.polygon([(p4[0], p4[1]), (p4[0] - 4, p4[1] - 6), (p4[0] + 4, p4[1] - 6)], fill=(80, 90, 110))

    out_path = os.path.join(ASSETS_DIR, "fig_3_1_architecture.png")
    img.save(out_path)
    print(f"Generated architecture diagram: {out_path}")

if __name__ == "__main__":
    print("Generating report figure assets...")
    # 1. Architecture diagram
    render_system_architecture_diagram()

    # 2. CLI demonstrations
    cargo_bin = "/home/topfloorboss/Desktop/KATANA/target/debug/katana"
    capture_command([cargo_bin, "doctor"], "katana doctor", "fig_4_1_doctor.png")
    capture_command([cargo_bin, "demo", "futex"], "katana demo futex", "fig_4_2_demo_futex.png")
    capture_command([cargo_bin, "demo", "io"], "katana demo io", "fig_4_3_demo_io.png")
    capture_command([cargo_bin, "demo", "writeback"], "katana demo writeback", "fig_4_4_demo_writeback.png")
    capture_command([cargo_bin, "demo", "watch"], "katana demo watch", "fig_4_5_demo_watch.png")
    capture_command([cargo_bin, "fixtures"], "katana fixtures", "fig_4_6_fixtures.png")
    capture_command([cargo_bin, "demo", "--bundle", "/tmp/demo.katana"], "katana demo --bundle /tmp/demo.katana", "fig_4_7_bundle.png")

    # 3. HTML export browser screenshots
    subprocess.run([cargo_bin, "demo", "futex", "--html", "/tmp/futex_report.html"], stdout=subprocess.DEVNULL)
    capture_browser_screenshot("/tmp/futex_report.html", "fig_4_8_html_futex.png")
    
    subprocess.run([cargo_bin, "demo", "io", "--html", "/tmp/io_report.html"], stdout=subprocess.DEVNULL)
    capture_browser_screenshot("/tmp/io_report.html", "fig_4_9_html_io.png")

    print("All figure assets generated successfully in:", ASSETS_DIR)
