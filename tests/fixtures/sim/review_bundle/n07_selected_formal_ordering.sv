module tb;
  typedef logic signed [7:0] matrix_t[0:1][0:2];
  matrix_t values;
  integer calls;
  integer result;

  function automatic integer pick(input integer row);
    calls = calls + 1;
    return row;
  endfunction

  function automatic integer private_copy(input matrix_t matrix);
    matrix[pick(0)].reverse();
    return matrix[0][0];
  endfunction

  task automatic copy_back(inout matrix_t matrix);
    matrix[pick(0)].sort();
  endtask

  task automatic by_reference(ref matrix_t matrix);
    matrix[pick(1)].rsort();
  endtask

  task automatic forward_reference(ref matrix_t matrix);
    by_reference(matrix);
  endtask

  initial begin
    values[0] = '{8'sd3, -8'sd1, 8'sd2};
    values[1] = '{8'sd6, 8'sd4, 8'sd5};
    calls = 0;
    result = private_copy(values);
    if (calls != 1 || result != 2 || values[0][0] !== 8'sd3 ||
        values[0][1] !== -8'sd1 || values[0][2] !== 8'sd2)
      $fatal(1, "input formal must mutate its private copy only");
    copy_back(values);
    if (calls != 2 || values[0][0] !== -8'sd1 ||
        values[0][1] !== 8'sd2 || values[0][2] !== 8'sd3 ||
        values[1][0] !== 8'sd6 || values[1][1] !== 8'sd4 || values[1][2] !== 8'sd5)
      $fatal(1, "inout copy-back receiver");
    forward_reference(values);
    if (calls != 3 || values[1][0] !== 8'sd6 ||
        values[1][1] !== 8'sd5 || values[1][2] !== 8'sd4 ||
        values[0][0] !== -8'sd1 || values[0][2] !== 8'sd3)
      $fatal(1, "forwarded reference receiver");
    $display("PASS n07_selected_formal_ordering");
    $finish(0);
  end
endmodule
