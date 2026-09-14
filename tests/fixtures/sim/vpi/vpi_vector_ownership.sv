// R16: vpiVectorVal returns simulator-owned, multiword four-state data.
module tb;
    logic [69:0] payload;
    initial begin
        payload = {6'b10xz01, 32'hfedcba98, 32'h76543210};
        $vpi_vector_probe(payload);
        $finish(0);
    end
endmodule
