// Decision EO-D4, streaming concatenations of record element members after a
// call: a queue or dynamic-array member of a record element of a queue or
// dynamic array (`rq[k].q`) that a streaming concatenation reads whole, cast
// to a packed value, after a call that changes `k` names the element
// selected after that call: `x = bump() + int'({>>{rq[k].q}})` reads
// `rq[1].q`, also right to left, by slices, inside a concatenation, in a
// condition and twice in one expression. A read before the call keeps the
// element the statement selects first.
//
// IEEE 1800-2009 11.4.2 (SystemVerilog-1800-2009.txt L14500): "The ordering
//   of assignment operations relative to any other operation within an
//   expression is undefined."
module tb;
  typedef struct { int q[$]; byte d[]; } r_t;
  typedef logic [15:0] h_t;
  typedef logic [31:0] w_t;
  r_t rq[$];
  r_t rd[];
  int k, x;
  h_t y;
  logic [63:0] w;
  function int bump();
    k = 1;
    return 10;
  endfunction
  initial begin
    r_t r0, r1;
    r0.q = '{1}; r0.d = new[2]; r0.d[0] = 8'h11; r0.d[1] = 8'h22;
    r1.q = '{3}; r1.d = new[2]; r1.d[0] = 8'h33; r1.d[1] = 8'h44;
    rq.push_back(r0); rq.push_back(r1);
    rd = new[2]; rd[0] = r0; rd[1] = r1;
    k = 0; x = bump() + int'({>>{rq[k].q}}); $display("stream x=%0d", x);
    k = 0; x = bump() + int'({<<{rq[k].q}}); $display("reverse x=%h", x);
    k = 0; y = h_t'(bump()) + h_t'({<<8{rd[k].d}}); $display("bytes y=%h", y);
    k = 0; x = int'({>>{rq[k].q}}) + bump(); $display("before x=%0d", x);
    k = 0; w = {w_t'(bump()), w_t'({>>{rq[k].q}})}; $display("concat w=%h", w);
    k = 0;
    if (bump() > 0 && int'({>>{rq[k].q}}) == 3) $display("cond yes");
    else $display("cond no");
    k = 0; x = bump() + int'({>>{rq[k].q}}) + int'({>>{rq[k].q}}); $display("twice x=%0d", x);
    $finish(0);
  end
endmodule
