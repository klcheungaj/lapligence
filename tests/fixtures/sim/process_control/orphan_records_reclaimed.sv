// llg-test-fixture: tests/fixtures/sim/process_control/orphan_records_reclaimed.sv
// IEEE 1800-2009 9.7 (L12631-12632) and 9.6.3 (L12553-12555): kill and
// disable fork reach the leaves below finished processes. Each iteration ends
// one tree by a kill of its live root, a disable fork, the leaves' own
// completion, or a kill of its finished root. The records kept for finished
// processes with live descendants are reclaimed with their trees: the run
// ends with one registered process (the `$finish(2)` statistics line).
`timescale 1ns/1ns
module tb;
  int fired;
  task automatic spawn(int gen);
    fork
      begin
        if (gen == 1) begin
          #50 fired++;
        end else begin
          spawn(gen - 1);
          #1;
        end
      end
    join_none
  endtask
  initial begin
    process root;
    for (int i = 0; i < 2000; i++) begin
      automatic int mode = i % 4;
      fork
        begin
          root = process::self();
          spawn(3);
          spawn(2);
          if (mode != 3) #100;
        end
      join_none
      #5;
      case (mode)
        0: root.kill();
        1: disable fork;
        2: #60 disable fork;
        default: root.kill();
      endcase
    end
    #100 $display("fired=%0d", fired);
    $finish(2);
  end
endmodule
