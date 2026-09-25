// IEEE 1800-2009 7.12.2: select one mutable receiver per method invocation.
module tb;
  integer calls;
  integer result;
  logic signed [7:0] collected[1:0][-1:-3];

  function automatic integer pick(input integer row);
    calls = calls + 1;
    return row;
  endfunction

  function automatic integer exercise();
    logic signed [7:0] matrix[1:0][-1:-3];
    logic [7:0] singleton[0:1][0:0];
    matrix[1] = '{8'sd3, -8'sd1, 8'sd2};
    matrix[0] = '{8'sd40, 8'sd50, 8'sd60};
    calls = 0;
    matrix[pick(1)].reverse();
    if (calls != 1 || matrix[1][-1] !== 8'sd2 ||
        matrix[1][-2] !== -8'sd1 || matrix[1][-3] !== 8'sd3)
      $fatal(1, "automatic reverse receiver");
    matrix[pick(1)].sort();
    if (calls != 2 || matrix[1][-1] !== -8'sd1 ||
        matrix[1][-2] !== 8'sd2 || matrix[1][-3] !== 8'sd3)
      $fatal(1, "automatic sort receiver");
    matrix[pick(1)].rsort();
    if (calls != 3 || matrix[1][-1] !== 8'sd3 ||
        matrix[1][-2] !== 8'sd2 || matrix[1][-3] !== -8'sd1)
      $fatal(1, "automatic rsort receiver");
    matrix[pick(1)].reverse();
    matrix[pick(1)].reverse();
    if (calls != 5 || matrix[1][-1] !== 8'sd3 ||
        matrix[1][-2] !== 8'sd2 || matrix[1][-3] !== -8'sd1 ||
        matrix[0][-1] !== 8'sd40 || matrix[0][-2] !== 8'sd50 ||
        matrix[0][-3] !== 8'sd60)
      $fatal(1, "automatic neighbors or reverse twice");
    singleton[0][0] = 8'd17;
    singleton[1][0] = 8'd23;
    singleton[pick(0)].sort();
    singleton[pick(0)].rsort();
    singleton[pick(0)].reverse();
    if (calls != 8 || singleton[0][0] !== 8'd17 || singleton[1][0] !== 8'd23)
      $fatal(1, "singleton receiver must still evaluate");
    return calls;
  endfunction

  initial begin
    result = exercise();
    if (result != 8) $fatal(1, "automatic call result");
    collected[1] = '{8'sd3, -8'sd1, 8'sd2};
    collected[0] = '{8'sd40, 8'sd50, 8'sd60};
    calls = 0;
    collected[pick(1)].reverse();
    collected[pick(1)].sort();
    collected[pick(1)].rsort();
    if (calls != 3 || collected[1][-1] !== 8'sd3 ||
        collected[1][-2] !== 8'sd2 || collected[1][-3] !== -8'sd1 ||
        collected[0][-1] !== 8'sd40 || collected[0][-3] !== 8'sd60)
      $fatal(1, "collected receiver control");
    $display("PASS n07_selected_automatic_ordering");
    $finish(0);
  end
endmodule
