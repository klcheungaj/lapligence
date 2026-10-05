// A procedural $sampled returns the value its expression had in the
// Preponed region of the current time slot (IEEE 1800-2009 16.9.3, 16.5.1),
// without any assertion or clock. At time 0 that is the declaration
// initializer, or the type's default: X for a variable, Z for a net.
module tb;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
  logic [7:0] x;
  logic [7:0] d = 8'h33;
  wire [7:0] w;
  logic signed [3:0] s;
  real r;
  pair_t [1:0] ps;
  integer i;

  assign w = x + 8'd1;

  initial begin
    x = 8'h5a;
    s = -4'sd2;
    r = 1.5;
    i = 0;
    ps = type(ps)'(16'h1234);
    $display("t0 %h %h %h %h", $sampled(x), $sampled(d), $sampled(w), $sampled(x[3:0]));
    #1 $display("t1 %h %h %0d %b %h", $sampled(x), $sampled(w), $sampled(s),
                $sampled(r > 1.0), $sampled(ps[i].hi));
    x = 8'h11;
    d = 8'h44;
    s = 4'sd3;
    r = 0.5;
    i = 1;
    $display("t1 after writes %h %h %h %0d %b %h live %h", $sampled(x), $sampled(d),
             $sampled(w), $sampled(s), $sampled(r > 1.0), $sampled(ps[i].hi), x);
    #0 $display("t1 next delta %h %h", $sampled(x), $sampled(w));
    #1 $display("t2 %h %h %h %0d %b %h", $sampled(x), $sampled(d), $sampled(w), $sampled(s),
                $sampled(r > 1.0), $sampled(ps[i].hi));
    $finish(0);
  end
endmodule
