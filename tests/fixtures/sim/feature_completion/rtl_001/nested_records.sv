// SV2009 sections 6.19, 7.2.2, 7.4, 7.6: fixed records, signed enums and copies.
module tb;
  typedef enum logic signed [3:0] { MINUS = -2, PLUS = 3 } mode_t;
  typedef struct { mode_t mode; logic [7:0] lane [1:0]; bit [1:0] flags; } item_t;
  typedef item_t matrix_t [2:1][-1:0];
  matrix_t original, copied;
  initial begin
    $display("defaults=%b:%h:%b", original[1][0].mode, original[1][0].lane[0], original[1][0].flags);
    foreach (original[i,j]) begin
      original[i][j].mode = MINUS;
      original[i][j].lane[1] = 8'h11;
      original[i][j].lane[0] = 8'h22;
      original[i][j].flags = 2'bxz;
    end
    copied = original;
    copied[2][-1].lane[0] = 8'haa;
    copied[1][0].mode = PLUS;
    $display("original=%0d:%h:%h:%b", original[2][-1].mode, original[2][-1].lane[1], original[2][-1].lane[0], original[2][-1].flags);
    $display("copied=%0d:%h:%h:%b other=%0d", copied[2][-1].mode, copied[2][-1].lane[1], copied[2][-1].lane[0], copied[2][-1].flags, copied[1][0].mode);
    copied[1:1] = original[2:2];
    $display("slice=%0d:%h untouched=%h", copied[1][0].mode, copied[1][0].lane[0], copied[2][-1].lane[0]);
    $finish(0);
  end
endmodule
