// SIM-032 A02: $exit in one program terminates all initials of that program
// and their descendants, including detached children forked by a module task
// it called, while another program continues (IEEE 1800-2009 24.3.1, 24.7).
module tb;
  int hits = 0;
  task automatic spawn_worker(input int id, input int d);
    fork
      begin
        #0 hits++;
        #d $display("worker %0d t=%0d", id, $time);
      end
    join_none
  endtask
  program pe;
    initial begin
      spawn_worker(1, 1);
      spawn_worker(2, 50);
      #10 $display("pe exits t=%0d hits=%0d", $time, hits);
      $exit;
      $display("pe after exit must not print");
    end
    initial begin
      #20 $display("pe second initial must not print");
    end
    final $display("pe final");
  endprogram
  program po;
    initial begin
      spawn_worker(3, 15);
      #30 $display("po ends t=%0d hits=%0d", $time, hits);
    end
    final $display("po final");
  endprogram
  final $display("tb final t=%0d", $time);
endmodule
