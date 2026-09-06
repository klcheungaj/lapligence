module tb;
    reg [63:0] drive_a;
    reg [63:0] drive_b;
    wire [127:0] bus;

    assign bus[79:16] = drive_a;
    assign bus[111:48] = drive_b;

    initial begin
        drive_a = {64{1'b1}};
        drive_b = {64{1'b0}};
        #1;
        if (bus[15] !== 1'bz || bus[16] !== 1'b1 || bus[47] !== 1'b1 ||
            bus[48] !== 1'bx || bus[79] !== 1'bx || bus[80] !== 1'b0 ||
            bus[111] !== 1'b0 || bus[112] !== 1'bz || bus[127] !== 1'bz) begin
            $display("FAIL selected_continuous_wire conflict");
            $finish;
        end

        drive_b = {64{1'b1}};
        #1;
        if (bus[16] !== 1'b1 || bus[48] !== 1'b1 ||
            bus[79] !== 1'b1 || bus[111] !== 1'b1) begin
            $display("FAIL selected_continuous_wire agree_one");
            $finish;
        end

        drive_a = {64{1'b0}};
        drive_b = {64{1'b0}};
        #1;
        if (bus[16] !== 1'b0 || bus[48] !== 1'b0 ||
            bus[79] !== 1'b0 || bus[111] !== 1'b0) begin
            $display("FAIL selected_continuous_wire agree_zero");
            $finish;
        end

        drive_a = {64{1'bz}};
        drive_b = {64{1'b1}};
        #1;
        if (bus[16] !== 1'bz || bus[47] !== 1'bz ||
            bus[48] !== 1'b1 || bus[79] !== 1'b1 || bus[111] !== 1'b1) begin
            $display("FAIL selected_continuous_wire high_impedance");
            $finish;
        end

        drive_a = {64{1'bx}};
        drive_b = {64{1'b0}};
        #1;
        if (bus[16] !== 1'bx || bus[47] !== 1'bx ||
            bus[48] !== 1'bx || bus[79] !== 1'bx || bus[111] !== 1'b0) begin
            $display("FAIL selected_continuous_wire unknown");
            $finish;
        end

        $display("PASS selected_continuous_wire");
        $finish;
    end
endmodule
