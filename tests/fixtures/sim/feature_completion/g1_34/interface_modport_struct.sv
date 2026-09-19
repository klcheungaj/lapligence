// llg-test-fixture: G1-34 rtl_composition_gate.
// One interface instance with complementary modports shared by a producer and
// a consumer. The interface carries a fixed packed structure; each side writes
// or reads the structure through its own modport view under resettable
// always_ff. The final observation depends on nonblocking assignment ordering:
// the consumer's output register holds the previous cycle's valid.
typedef struct packed {
    logic [7:0]  tag;
    logic [15:0] payload;
} g1_frame_t;

interface g1_bus_if (input logic clk);
    g1_frame_t wdata;
    logic      wvalid;
    g1_frame_t rdata;
    logic      rvalid;
    modport driver (input clk, output wdata, output wvalid);
    modport receiver (input clk, input wdata, input wvalid,
                      output rdata, output rvalid);
endinterface

module g1_producer (
    g1_bus_if.driver b,
    input  logic        rst_n,
    input  logic [7:0]  tag,
    input  logic [15:0] payload,
    input  logic        push
);
    always_ff @(posedge b.clk) begin
        if (!rst_n) begin
            b.wdata <= '0;
            b.wvalid <= 1'b0;
        end else if (push) begin
            b.wdata.tag     <= tag;
            b.wdata.payload <= payload;
            b.wvalid        <= 1'b1;
        end else begin
            b.wvalid <= 1'b0;
        end
    end
endmodule

module g1_consumer (
    g1_bus_if.receiver b,
    input  logic        rst_n,
    output g1_frame_t   out_frame,
    output logic        out_valid
);
    always_ff @(posedge b.clk) begin
        if (!rst_n) begin
            b.rdata <= '0;
            b.rvalid <= 1'b0;
        end else if (b.wvalid) begin
            b.rdata.tag     <= b.wdata.tag + 8'd1;
            b.rdata.payload <= b.wdata.payload + 16'h0101;
            b.rvalid        <= 1'b1;
        end else begin
            b.rvalid <= 1'b0;
        end
    end
    always_ff @(posedge b.clk) begin
        if (!rst_n) begin
            out_frame <= '0;
            out_valid <= 1'b0;
        end else begin
            out_frame <= b.rdata;
            out_valid <= b.rvalid;
        end
    end
endmodule

module tb;
    logic clk;
    logic rst_n;
    logic [7:0]  tag;
    logic [15:0] payload;
    logic        push;
    g1_frame_t   out_frame;
    logic        out_valid;

    g1_bus_if bus (.clk(clk));
    g1_producer u_prod (.b(bus.driver), .rst_n(rst_n), .tag(tag),
                        .payload(payload), .push(push));
    g1_consumer u_cons (.b(bus.receiver), .rst_n(rst_n),
                        .out_frame(out_frame), .out_valid(out_valid));

    initial begin
        clk = 0;
        rst_n = 0;
        tag = 8'h2a;
        payload = 16'hbeef;
        push = 1'b1;
        #1 clk = 1;
        #1 clk = 0;
        rst_n = 1;
        $display("reset wv=%b rv=%b rt=%02h rp=%04h ov=%b of=%02h%04h",
                 bus.wvalid, bus.rvalid, bus.rdata.tag, bus.rdata.payload,
                 out_valid, out_frame.tag, out_frame.payload);
        #1 clk = 1;
        #1 clk = 0;
        $display("t1 wv=%b rv=%b rt=%02h rp=%04h ov=%b of=%02h%04h",
                 bus.wvalid, bus.rvalid, bus.rdata.tag, bus.rdata.payload,
                 out_valid, out_frame.tag, out_frame.payload);
        push = 1'b0;
        #1 clk = 1;
        #1 clk = 0;
        $display("t2 wv=%b rv=%b rt=%02h rp=%04h ov=%b of=%02h%04h",
                 bus.wvalid, bus.rvalid, bus.rdata.tag, bus.rdata.payload,
                 out_valid, out_frame.tag, out_frame.payload);
        #1 clk = 1;
        #1 clk = 0;
        $display("t3 wv=%b rv=%b rt=%02h rp=%04h ov=%b of=%02h%04h",
                 bus.wvalid, bus.rvalid, bus.rdata.tag, bus.rdata.payload,
                 out_valid, out_frame.tag, out_frame.payload);
        $finish(0);
    end
endmodule
