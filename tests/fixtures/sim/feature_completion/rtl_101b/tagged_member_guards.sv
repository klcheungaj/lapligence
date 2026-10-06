// RTL-101b: whole members of a column-layout tagged union are checked
// against the tag like element accesses.
module tb;
  typedef struct { logic [7:0] a [0:262143]; logic [3:0] k; } rec_t;
  typedef union tagged {
    logic [1023:0] w [0:2047];
    rec_t s;
    bit [7:0] t;
  } tu_t;
  tu_t v;
  logic [1023:0] arr [0:2047];
  logic [1023:0] brr [0:2047];
  rec_t x, y;
  initial begin
    arr[0] = 1024'd1;
    brr[0] = 1024'd9;
    x.k = 4'h5;
    x.a[2] = 8'h7;
    y.k = 4'h3;
    v = tagged t 8'd4;
    brr = v.w;
    $display("A %0d %0d", brr[0][7:0], v.t);
    v.w = arr;
    $display("B %0d", v.t);
    $display("C %0d", v.w == arr);
    y = v.s;
    $display("D %h %h", y.k, y.a[2]);
    v.s = x;
    $display("E %0d", v.t);
    v = tagged w arr;
    brr = v.w;
    $display("F %0d %0d", brr[0][7:0], v.w[0] == arr[0]);
    v = tagged s x;
    y = v.s;
    $display("G %h %h", y.k, y.a[2]);
    $finish(0);
  end
endmodule
