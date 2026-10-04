// SIM-003: descriptor-backed native record formals, results and locals.
// IEEE 1800-2009 6.11.2, 6.14, 6.16, 7.2, 7.2.2, 13.3-13.5; see readme.md.
`timescale 1ns/1ns
module tb;
  typedef enum logic [1:0] {IDLE, BUSY, DONE} state_t;
  typedef struct packed {logic [3:0] hi; logic [3:0] lo;} nib_t;
  typedef struct {string tag; chandle h;} meta_t;
  typedef struct {
    meta_t meta;
    real weight;
    shortreal ratio;
    int count;
    bit [7:0] flags;
    logic [7:0] raw;
    nib_t nib;
    state_t st;
    string names [0:2];
  } rec_t;

  rec_t a, b, t1, t2;
  meta_t m;
  int calls;

  // Every call builds its result in a fresh automatic local.
  function automatic rec_t build(input string tag, input int count);
    rec_t r;
    r.meta.tag = tag;
    r.meta.h = null;
    r.weight = 1.5 * real'(count);
    r.ratio = 0.1;
    r.count = count;
    r.flags = 8'b0000_x1z1;
    r.nib = 8'h5a;
    r.nib.lo = 4'h3;
    r.st = BUSY;
    r.names[0] = {tag, "0"};
    r.names[2] = {tag, "2"};
    return r;
  endfunction

  // An input formal is the callee's own copy.
  function automatic int scribble(input rec_t r);
    r.meta.tag = "scribbled";
    r.count = -1;
    r.names[1] = "x";
    return r.count + r.meta.tag.len();
  endfunction

  // Recursive activations each own their local record.
  function automatic int depth(input rec_t r, input int k);
    rec_t next;
    if (k == 0) return r.count;
    next = r;
    next.count = r.count + k;
    next.meta = r.meta;
    return depth(next, k - 1);
  endfunction

  // Native inout/output formals survive suspension and copy out on return.
  task automatic update(inout rec_t r, output meta_t mo, input int delta);
    #2;
    r.count = r.count + delta;
    r.names[1] = "updated";
    mo = r.meta;
    mo.tag = {mo.tag, "!"};
  endtask

  // Static subroutine storage persists between calls.
  function rec_t tally(input rec_t r);
    static rec_t last;
    calls++;
    last.count = last.count + r.count;
    last.meta.tag = {last.meta.tag, r.meta.tag};
    return last;
  endfunction

  function automatic logic same(input rec_t x, input rec_t y);
    return x == y;
  endfunction

  // Case equality is not defined for real members (11.4.5); compare metadata.
  function automatic bit ident(input meta_t x, input meta_t y);
    return x === y;
  endfunction

  function automatic string label(input meta_t mm);
    return {mm.tag, (mm.h == null) ? "-" : "+"};
  endfunction

  initial begin
    a = build("alpha", 4);
    b = a;
    a.meta.tag = "changed";
    a.names[0] = "z";
    $display("%s %s %.2f %0d %h %h %h %h %s [%s|%s|%s] %0d", b.meta.tag,
             (b.meta.h == null) ? "null" : "set", b.weight, b.count, b.flags,
             b.raw, b.nib, b.nib[7:4], b.st.name(), b.names[0], b.names[1],
             b.names[2], b == a);
    $display("%.10f", b.ratio);
    $display("%0d %s [%s]", scribble(b), b.meta.tag, b.names[1]);
    $display("%0d %0d", depth(b, 3), b.count);
    update(b, m, 5);
    $display("%0t %0d %s %s %s", $time, b.count, b.names[1], m.tag, b.meta.tag);
    t1 = tally(build("x", 2));
    t2 = tally(build("y", 3));
    $display("%0d %0d %s %0d %s", calls, t1.count, t1.meta.tag, t2.count, t2.meta.tag);
    t1 = b;
    $display("%b %b %b", same(t1, b), same(t1, a), ident(t1.meta, b.meta));
    t1.names[2] = "other";
    $display("%b %b %b", same(t1, b), t1 != b, ident(t1.meta, a.meta));
    $display("%s %s", label('{"pat", null}), label(b.meta));
    $finish(0);
  end
endmodule
