module tb;
  typedef logic [7:0] matrix_t[0:1][0:2];
  matrix_t values;
  task automatic invalid(const ref matrix_t matrix);
    matrix[0].reverse();
  endtask
  initial invalid(values);
endmodule
