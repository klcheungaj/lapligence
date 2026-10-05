// An automatic row cannot take a nonblocking pattern write (IEEE 1800-2009
// 6.21, 10.4.2), whether the source is small or descriptor storage.
module tb;
  logic [7:0] w [2][4];
  logic [7:0] big [2000][4];
  initial begin
    automatic logic [7:0] r [4];
    '{big[1], r} <= w;
  end
endmodule
