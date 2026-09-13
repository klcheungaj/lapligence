`timescale 1ns/1ps
module tb;
  event deferred;
  event immediate;
  event delayed;
  event gate;
  event controlled;
  event pulse;
  event repeated;
  event mixed;
  reg signal;
  integer stage;
  integer mixed_wakes;

  initial begin
    stage = 0;
    ->> deferred;
    stage = 1;
    @(deferred);
    $display("CHECK: deferred stage=%0d", stage);
  end

  initial begin
    -> immediate;
    fork
      begin
        @immediate;
        $display("CHECK: immediate woke=1");
      end
      begin
        #1;
        $display("CHECK: immediate woke=0");
      end
    join_any
    disable fork;
  end

  initial begin
    ->> #2 delayed;
  end

  initial begin
    @delayed;
    $display("CHECK: delayed time=%0t", $time);
  end

  initial begin
    ->> @gate controlled;
    $display("CHECK: controlled caller=running");
  end

  initial begin
    @controlled;
    $display("CHECK: controlled time=%0t", $time);
  end

  initial begin
    ->> repeat (1 + 1) @pulse repeated;
    $display("CHECK: repeated caller=running");
  end

  initial begin
    @repeated;
    $display("CHECK: repeated time=%0t", $time);
  end

  initial begin
    mixed_wakes = 0;
    fork
      begin
        @(signal or mixed);
        mixed_wakes = mixed_wakes + 1;
      end
      begin
        #1 signal = 1'b1;
      end
      begin
        #2 -> mixed;
      end
    join_none
    #3;
    $display("CHECK: mixed wakes=%0d", mixed_wakes);
  end

  initial begin
    #1 -> gate;
    #1 -> pulse;
    #1 -> pulse;
  end

  initial begin
    #4 $finish(0);
  end
endmodule
