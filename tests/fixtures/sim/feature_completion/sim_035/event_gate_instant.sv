// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_035/event_gate_instant.sv
// SIM-035: the `$past` gate of an event clock that ticks through a waiting
// process (event list, named event, expression edge) is read when the event
// occurs (IEEE 1800-2009 16.9.3 "ev iff expression2", 9.4.2.3), even when the
// same statement sequence clears the gate right after the event. See readme.
module tb;
  logic a = 1'b0, b = 1'b0, c = 1'b0, g = 1'b1;
  logic [3:0] v = 4'd1;
  event ev;

  initial begin
    #1 v = 4'd2;
    #1 begin
      a = 1'b1;
      g = 1'b0;
    end
    #1 v = 4'd3;
    #1 $display("L %0d %0d", $past(v, 1, g, @(posedge a or posedge b)),
                $past(v, 1, g, @(posedge (a & c))));
    #1 begin
      v = 4'd5;
      g = 1'b1;
    end
    #1 begin
      ->ev;
      c = 1'b1;
      g = 1'b0;
    end
    #1 v = 4'd7;
    #1 $display("N %0d %0d", $past(v, 1, g, @(ev)), $past(v, 1, g, @(posedge (a & c))));
    #1 begin
      g = 1'b1;
      ->ev;
    end
    #1 $display("G %0d", $past(v, 1, g, @(ev)));
    $finish;
  end
endmodule
