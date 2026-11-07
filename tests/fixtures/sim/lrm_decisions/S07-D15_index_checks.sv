// Decision S07-D15: index checks. With `simulator.index_checks` on (the
// default), each case below stops the run with an error, as some simulators
// do; with it off (`--no-index-checks`) the text applies: a read gives the
// default, a write and a queue method change nothing, a `$cast` writes nothing
// and returns 0, and a `ref` actual binds the default. `+case=N` runs case N,
// no plusarg runs them all.
//
// IEEE 1800-2009 7.4.6 (SystemVerilog-1800-2009.txt L8234-8238): "If an index
// expression is out of the address bounds or if any bit in the address is X or
// Z, then the index shall be invalid. The result of reading from an array with
// an invalid index shall return the default uninitialized value for the array
// element type. Writing to an array with an invalid index shall perform no
// operation." 7.8.6 (L8720-8723, L8756-8757): an X key read warns and returns
// the default; "If an invalid index is used during a write operation, the
// write shall be ignored, and a warning shall be issued."
module tb;
  class base; endclass
  class item extends base; endclass
  typedef enum {RED, GREEN, BLUE} color_t;
  int q[$];
  int aq[logic [3:0]];
  event ev[logic [3:0]];
  item fa[4];
  item cq[$];
  item cd[];
  item ca[logic [3:0]];
  color_t ef[4];
  int iq[$];
  int ia[logic [3:0]];
  int c, k, r, v;
  logic [3:0] x, z;
  base b;
  task automatic bump(ref int value);
    value = value + 1;
  endtask
  function automatic bit run(int n);
    return c == 0 || c == n;
  endfunction
  initial begin
    item o;
    int e;
    if (!$value$plusargs("case=%d", c)) c = 0;
    q = '{1, 2};
    aq[1] = 1;
    o = new;
    b = o;
    cd = new[2];
    iq = '{1, 2};
    ia[1] = 1;
    k = 5;
    x = 4'bx;
    z = 4'bz;
    e = 2;
    if (run(1)) begin q.insert(k, 9); $display("1 %p", q); end
    if (run(2)) begin q.insert(x, 9); $display("2 %p", q); end
    if (run(3)) begin q.delete(-k); $display("3 %p", q); end
    if (run(4)) begin v = aq[x]; $display("4 %0d %0d", v, aq.num()); end
    if (run(5)) begin aq[z] = 7; $display("5 %0d", aq.num()); end
    if (run(6)) begin -> ev[x]; $display("6 %0d", ev.num()); end
    if (run(7)) begin r = $cast(fa[k], b); $display("7 %0d", r); end
    if (run(8)) begin r = $cast(ef[-k], e); $display("8 %0d %p", r, ef); end
    if (run(9)) begin r = $cast(cq[x], b); $display("9 %0d %0d", r, cq.size()); end
    if (run(10)) begin r = $cast(cd[z], b); $display("10 %0d", r); end
    if (run(11)) begin r = $cast(ca[x], b); $display("11 %0d %0d", r, ca.num()); end
    if (run(12)) begin bump(iq[k]); $display("12 %p", iq); end
    if (run(13)) begin bump(ia[x]); $display("13 %0d", ia.num()); end
    $display("end");
  end
endmodule
