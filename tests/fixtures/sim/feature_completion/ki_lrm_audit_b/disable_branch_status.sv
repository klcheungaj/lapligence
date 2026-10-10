// LRM audit B, N3 (SV 9.7, 9.6.2): disabling the named block that forms a
// whole fork branch ends that process KILLED.
`timescale 1ns / 1ns
module tb;
  process other, self_d, part, normal, joined;
  int after;

  initial begin
    after = 0;
    fork
      begin : other_blk
        other = process::self();
        #10;
      end
      begin : self_blk
        self_d = process::self();
        #1 disable self_blk;
        $display("unexpected self");
      end
      begin
        part = process::self();
        begin : part_blk
          #10;
        end
        after = 1;
      end
      begin : normal_blk
        normal = process::self();
        #1;
      end
    join_none
    #1 disable other_blk;
    disable part_blk;
    fork
      begin : joined_blk
        joined = process::self();
        #10;
      end
      #1 disable joined_blk;
    join
    $display("%0d: joined %s", $time, joined.status().name());
    #1 $display("other %s self %s part %s after=%0d normal %s", other.status().name(),
                self_d.status().name(), part.status().name(), after, normal.status().name());
    $finish(0);
  end
endmodule
