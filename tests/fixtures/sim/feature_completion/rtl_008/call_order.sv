// SV2009 6.8, 6.21, 10.5, 13.4: a static initializer that calls a function is
// still ordered with its declaration; later initializers observe its value.
// Unpacked-array and record initializers mix call results and earlier
// declarations.
module tb;
  typedef struct { int x; logic [7:0] y; } rec_t;
  function int seven();
    return 7;
  endfunction
  int a = seven();
  int b = a + 1;
  int arr [0:2] = '{a, seven(), b};
  int c = arr[0] + arr[1] + arr[2];
  rec_t rec = '{c * 2, 8'(b)};
  int d = rec.x + rec.y;
  initial begin
    $display("%0d %0d %0d %0d %0d %0d", a, b, arr[0], arr[1], arr[2], c);
    $display("%0d %0d %0d", rec.x, rec.y, d);
    $finish(0);
  end
endmodule
