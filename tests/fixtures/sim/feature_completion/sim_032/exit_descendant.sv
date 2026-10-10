// SIM-032 A02: $exit from a grandchild thread of a program initial terminates
// that program's whole thread tree (IEEE 1800-2009 24.7).
program pe;
  initial begin
    fork
      begin
        fork
          begin
            #3 $display("grandchild exits t=%0d", $time);
            $exit;
          end
          begin
            #8 $display("grandchild sibling must not print");
          end
        join_none
        #9 $display("child must not print");
      end
    join_none
    #7 $display("parent must not print");
  end
endprogram

program po;
  initial #12 $display("po continues t=%0d", $time);
endprogram

module tb;
  pe e0();
  po o0();
  final $display("final t=%0d", $time);
endmodule
