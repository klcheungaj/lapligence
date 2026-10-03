// Unsupported boundary: force evaluators are runtime callbacks; a helper with visible writes is rejected.
module tb;
  logic [7:0] x, y;
  int cnt = 0;
  function automatic logic [7:0] counted(input logic [7:0] v);
    cnt++;
    return v + 1;
  endfunction
  initial begin
    x = 1;
    force y = counted(x);
    #1 $display("%0d", y);
    $finish(0);
  end
endmodule
