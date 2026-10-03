// IEEE 1800-2009 §10.3: delayed sites retain independent selected-cell updates.
module tb;
    logic [7:0] values [0:16777215];
    logic [7:0] first_drive = 1;
    logic [7:0] last_drive = 2;
    assign #2 values[0] = first_drive;
    assign #3 values[16777215] = last_drive;
    initial begin
        #1;
        first_drive = 3;
        #1;
        if (values[0] !== 8'bx) $fatal;
        #2;
        if (values[0] !== 3 || values[16777215] !== 2 || values[1] !== 8'bx) $fatal;
        last_drive = 4;
        #4;
        if (values[0] !== 3 || values[16777215] !== 4) $fatal;
        $display("PASS rtl002 inertial");
        $finish(0);
    end
endmodule
