// SYN-038 shape witness: a three-dimensional fixed array of fixed records.
module tb;
  typedef struct packed { logic [6:0] value; bit flag; } cell_t;
  cell_t grid[1:0][-1:0][2:3];
  initial begin
    grid[1][-1][2] = '{value:7'd7, flag:1'b0};
    grid[1][-1][3] = '{value:7'd14, flag:1'b1};
    grid[1][0][2] = '{value:7'd21, flag:1'b0};
    grid[1][0][3] = '{value:7'd28, flag:1'b1};
    grid[0][-1][2] = '{value:7'd35, flag:1'b0};
    grid[0][-1][3] = '{value:7'd42, flag:1'b1};
    grid[0][0][2] = '{value:7'd49, flag:1'b0};
    grid[0][0][3] = '{value:7'd56, flag:1'b1};
    if (grid[1][-1][2].value !== 7'd7 ||
        grid[0][0][3].value !== 7'd56 ||
        grid[1][-1][3].flag !== 1'b1)
      $fatal(1, "three-dimensional record array");
    $display("record_grid3d=%0d,%0d flags=%b,%b",
             grid[1][-1][2].value, grid[0][0][3].value,
             grid[1][-1][2].flag, grid[0][0][3].flag);
    $finish;
  end
endmodule
