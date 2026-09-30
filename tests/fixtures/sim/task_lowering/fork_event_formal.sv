module tb;
  timeunit 1ns;
  timeprecision 1ns;
  event go;
  integer hits = 0;
  task automatic waiter(input event e);
    fork
      begin @(e); hits = hits + 1; end
      begin #1; end
    join
  endtask
  initial begin
    $timeformat(-9, 0, "", 0);
    waiter(go);
    $display("hits=%0d t=%0t", hits, $time);
    $finish(0);
  end
  initial begin #2; -> go; end
endmodule
