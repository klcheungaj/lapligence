// Whole-value reads and writes of 4096-element dense arrays: the generated C
// must not grow with the element count (see readme.md).
typedef struct { logic [3:0] a; int i; } rec_t;
typedef rec_t recs_t [0:4095];

module tb;
  recs_t ra, rb, rc;
  logic [7:0] bytes [0:4095], copy [0:4095];
  logic c;
  string text;

  task automatic pass(input recs_t x, output recs_t y);
    y = x;
  endtask

  initial begin
    foreach (ra[k]) ra[k] = '{4'(unsigned'(k)), k};
    rb = ra;
    c = 1'bx;
    rc = c ? ra : rb;
    pass(rc, rb);
    foreach (bytes[k]) bytes[k] = 8'(unsigned'(k));
    copy = bytes;
    text = $sformatf("%p", copy);
    $display("%0d %0d %h %0d %b %b %0d", rb[4095].i, rc[17].i, rc[17].a, copy[4095],
             ra == rb, bytes === copy, text.len());
    $finish(0);
  end
endmodule
