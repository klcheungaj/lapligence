// SIM-010: automatic strings of a task are shared with its join_none
// branches through the activation frame (each side sees the other's writes),
// copied into joined branches, and fork block item strings are per fork
// execution (SV 6.21, 9.3.2).
module tb;
    task automatic t();
        string s = "a";
        string keep = "k";
        fork
            begin
                #1 $display("child %s %s", s, keep);
                s = {s, "!"};
            end
        join_none
        s = "b";
        #2 $display("parent %s", s);
    endtask
    task automatic copy_only();
        string m = "m";
        fork
            begin
                string mine = m;
                #1 $display("copy %s", mine);
            end
        join
    endtask
    initial begin
        t();
        copy_only();
    end
endmodule
