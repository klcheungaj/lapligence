module tb;
  typedef struct { logic [3:0] a; logic [3:0] b; } pair_t;
  pair_t pair;
  initial begin
    assign pair = '{4'h1, 4'h2};
    #1 $display("%h %h", pair.a, pair.b);
    $finish;
  end
endmodule
