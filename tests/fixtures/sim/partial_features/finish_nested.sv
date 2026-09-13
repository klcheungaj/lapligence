module tb;
  task automatic stop_task;
    $display("CHECK: task before");
    stop_function();
    $display("CHECK: task after");
  endtask

  function automatic void stop_function;
    $display("CHECK: function before");
    $finish(0);
    $display("CHECK: function after");
  endfunction

  initial begin
    stop_task();
    $display("CHECK: initial after");
  end

  final $display("CHECK: final");
endmodule
