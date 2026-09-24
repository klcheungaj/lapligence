// IEEE 1800-2009 §9.2.2.3–9.2.2.4: both process declarations are interface
// body items, and each process writes a scalar interface member.
//
// SYN038 selected paths:
//   always_latch: TY=integral_bit_logic, OP=direct_projection,
//     CO=assignment_rhs, LV=whole_object, SL=interface_member, FM=none,
//     HC=interface, HR=local, CP=none, CT=none, IN=none,
//     WK=procedural_blocking, PC=always_latch.
//   always_ff: TY=integral_bit_logic, OP=direct_projection,
//     CO=assignment_rhs, LV=whole_object, SL=interface_member, FM=none,
//     HC=interface, HR=local, CP=none, CT=none, IN=none,
//     WK=procedural_nba, PC=always_ff.
interface process_if;
    logic latch_enable;
    logic [7:0] latch_data;
    logic [7:0] latched_value;

    logic clk;
    logic reset_n;
    logic [7:0] ff_data;
    logic [7:0] registered_value;
    logic [7:0] captured_value;

    always_latch begin
        if (latch_enable)
            latched_value = latch_data;
    end

    always_ff @(posedge clk or negedge reset_n) begin
        if (!reset_n) begin
            registered_value <= 8'h00;
            captured_value <= 8'h00;
        end else begin
            registered_value <= ff_data;
            captured_value <= registered_value;
        end
    end
endinterface

module tb;
    process_if p();

    initial begin
        p.latch_enable = 1'b0;
        p.latch_data = 8'h00;
        p.clk = 1'b0;
        p.reset_n = 1'b1;
        p.ff_data = 8'h00;

        #1 p.reset_n = 1'b0;
        #1 begin
            p.reset_n = 1'b1;
            p.latch_enable = 1'b1;
            p.latch_data = 8'h3c;
            p.ff_data = 8'h5a;
        end

        #1 begin
            $display("latch open=%h", p.latched_value);
            p.latch_enable = 1'b0;
            p.latch_data = 8'ha5;
            p.clk = 1'b1;
        end
        #0 $display(
            "ff issue q=%h captured=%h", p.registered_value, p.captured_value);
        #1 begin
            $display("latch held=%h", p.latched_value);
            $display(
                "ff commit q=%h captured=%h", p.registered_value, p.captured_value);
            p.clk = 1'b0;
            p.ff_data = 8'hc3;
        end

        #1 p.clk = 1'b1;
        #0 $display(
            "ff issue q=%h captured=%h", p.registered_value, p.captured_value);
        #1 begin
            $display(
                "ff commit q=%h captured=%h", p.registered_value, p.captured_value);
            $finish(0);
        end
    end
endmodule
