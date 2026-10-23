// Whole-value reads and writes of arrays above the dense-storage threshold
// (4096 cells), which use lazily allocated descriptor storage: the
// generated C must not grow with the element count (see readme.md).
typedef struct { logic [3:0] a; int i; } rec_t;
typedef rec_t recs_t [0:4999];
typedef logic [7:0] bytes_t [1:5000];

module tb;
  recs_t ra, rb;
  bytes_t bytes;
  string text;

  task automatic pass(input recs_t x, output recs_t y);
    y = x;
  endtask

  task automatic bump(inout bytes_t v);
    foreach (v[k]) v[k] = v[k] + 8'd1;
  endtask

  initial begin
    ra[0] = '{4'h1, 10};
    ra[4999] = '{4'hz, -2};
    ra[2500].i = 7;
    pass(ra, rb);
    $display("pass %h %0d %h %0d %h %0d %h %0d %b", rb[0].a, rb[0].i, rb[2500].a,
             rb[2500].i, rb[4999].a, rb[4999].i, rb[17].a, rb[17].i, ra === rb);
    for (int unsigned k = 1000; k <= 5000; k += 1000) bytes[k] = 8'(k / 1000);
    bump(bytes);
    text = $sformatf("%p", bytes);
    $display("bump %0d %0d %h %0d %s", bytes[1000], bytes[5000], bytes[1],
             text.len(), text.substr(text.len() - 8, text.len() - 1));
    $finish(0);
  end
endmodule
