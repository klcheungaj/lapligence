// llg-test-fixture: an ordinary (unresolved) net with conflicting drivers
// exposes X at its observation point, while a wired-AND net with the same
// drivers applies its resolution rule. LRM: IEEE 1800-2009 6.6.6-6.7.
module tb;
    reg a, b;
    wire conflict;
    wand wired;

    assign conflict = a;
    assign conflict = b;
    assign wired = a;
    assign wired = b;

    initial begin
        a = 1'b0;
        b = 1'b1;
        #1;
        $display("CHECK: conflict=%b wired=%b", conflict, wired);
        a = 1'b1;
        b = 1'b1;
        #1;
        $display("CHECK: conflict=%b wired=%b", conflict, wired);
        a = 1'bz;
        b = 1'bz;
        #1;
        $display("CHECK: conflict=%b wired=%b", conflict, wired);
        $finish(0);
    end
endmodule
