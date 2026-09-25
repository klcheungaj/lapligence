module wired_target(inout wand a, inout wor o);
    reg local_a, local_o;
    assign a=local_a;
    assign o=local_o;
endmodule
module wired_sibling;
    reg drive_a, drive_o;
    assign tb.target.a=drive_a;
    assign tb.target.o=drive_o;
endmodule
module tb;
    wand a;
    wor o;
    reg parent_a, parent_o;
    wired_target target(a,o);
    wired_sibling source();
    assign a=parent_a;
    assign o=parent_o;
    initial begin
        target.local_a=1; target.local_o=0;
        parent_a=1'bz; parent_o=1'bz;
        source.drive_a=0; source.drive_o=1;
        #1;
        if (a !== 0 || target.a !== 0 || o !== 1 || target.o !== 1) begin
            $display("FAIL upward collapsed drivers"); $finish(0);
        end
        source.drive_a=1'bz; source.drive_o=1'bz;
        #1;
        if (a !== 1 || o !== 0) begin $display("FAIL independent removal"); $finish(0); end
        target.local_a=1'bz; target.local_o=1'bz;
        parent_a=1'bx; parent_o=1'bx;
        #1;
        if (a !== 1'bx || o !== 1'bx) begin $display("FAIL unknown remaining driver"); $finish(0); end
        parent_a=1'bz; parent_o=1'bz;
        #1;
        if (a !== 1'bz || o !== 1'bz) begin $display("FAIL floating collapse"); $finish(0); end
        $display("WIRED_UPWARD_PORTS_PASS");
        $finish(0);
    end
endmodule
