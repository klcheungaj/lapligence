// SIM-017: unpacked fixed-array messages built from assignment patterns and
// received into a record member of the array type (SV 15.4, 10.9, 7.4).
module tb;
  typedef int quad_t[4];
  typedef struct {
    quad_t quad;
    string tag;
  } holder_t;

  mailbox #(quad_t) m = new();
  holder_t h;

  initial begin
    h.tag = "h";
    m.put('{1, 2, 3, 4});
    m.put('{default: 7});
    m.get(h.quad);
    $display("%0d %0d %0d %0d %s", h.quad[0], h.quad[1], h.quad[2], h.quad[3], h.tag);
    if (m.try_peek(h.quad)) $display("%0d %0d n=%0d", h.quad[0], h.quad[3], m.num());
    $finish;
  end
endmodule
