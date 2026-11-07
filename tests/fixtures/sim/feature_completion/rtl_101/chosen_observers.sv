// A cell write publishes the array's contents marker only when the model
// reads the marker. Run-time index readers keep waking through it, in every
// instance of a shared body, beside a record nothing watches.
`timescale 1ns / 1ns

typedef struct {
  logic [7:0] tag;
  int a[16];
} r_t;

module m (input int k, input int v);
  r_t r, quiet;
  int seen;

  always_comb seen = r.a[k];
  always @(seen) $display("%0t %m seen=%0d", $time, seen);

  initial begin
    wait (r.a[k] == 9);
    $display("%0t %m wait done", $time);
  end

  always @(v) begin
    r.a[k] = v;
    quiet.a[k] = v + 1;
  end
endmodule

module tb;
  int k = 2, v1 = 0, v2 = 0;
  r_t t, w;
  m u1 (k, v1);
  m u2 (k, v2);

  always @(t.a[k]) $display("%0t t.a[k]=%0d", $time, t.a[k]);

  initial begin
    $monitor("%0t monitor %0d", $time, w.a[k]);
    #1 v2 = 4;
    #1 v1 = 9;
    #1 k = 3;
    #1 v2 = 9;
    #1 w.a[3] = 6;
    #1 t = w;
    #1 w.a[3] = 7;
    #1 t = w;
    #1 t.a[3] = 7;
    #1 $finish(0);
  end
endmodule
