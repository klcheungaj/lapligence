module tb;
  wire w;
  reg [1:0] mem [0:3];
  reg driven;
  integer n;
  real s;
  wire a, b;
  assign w = driven;
  assign {a, b} = $getpattern(mem[0]);
  initial begin
    driven = 1'b1;
    mem[0] = 2'b10;
    n = $countdrivers(w);
    s = $scale(tb);
    $scope(tb);
    $showscopes;
    $showvars;
    $showvars(w);
    $display("%0d %f", n, s);
    $finish;
  end
endmodule
