// Ordinary procedural static functions must still retain unassigned results.
module tb;
  function bit [7:0] by_break(input bit write_it);
    do begin
      if (!write_it) break;
      by_break = 8'd7;
    end while (0);
  endfunction
  function bit [7:0] by_continue(input bit write_it);
    do begin
      if (!write_it) continue;
      by_continue = 8'd9;
    end while (0);
  endfunction
  initial begin
    if (by_break(1) !== 7) $fatal(1, "initial break result");
    if (by_break(0) !== 7) $fatal(1, "static break result lost");
    if (by_continue(1) !== 9) $fatal(1, "initial continue result");
    if (by_continue(0) !== 9) $fatal(1, "static continue result lost");
    $display("PASS n02_static_do_state_control");
    $finish(0);
  end
endmodule
