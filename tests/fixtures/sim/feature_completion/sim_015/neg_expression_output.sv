// SIM-015: an output process formal bound to a process variable needs a
// copy-back after the call; a call inside an expression has no statement to
// run it and is rejected explicitly (SV 13.5).
module tb;
  function automatic int grab(output process p);
    p = process::self();
    return 1;
  endfunction

  int x;
  initial begin
    process q;
    x = grab(q) + 1;
    $display("%0d", x);
    $finish;
  end
endmodule
