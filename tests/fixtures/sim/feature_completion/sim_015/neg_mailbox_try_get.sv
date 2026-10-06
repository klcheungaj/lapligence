// SIM-015: try_get into a process variable needs a counted copy-back that an
// expression cannot run; it is rejected explicitly (get and peek work).
module tb;
  mailbox #(process) typed;
  process got;
  initial begin
    typed = new;
    void'(typed.try_put(process::self()));
    if (typed.try_get(got)) $display("got");
    $finish;
  end
endmodule
