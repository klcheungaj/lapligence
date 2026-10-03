// IEEE 1800-2009 §11.4.14: independent bit-position oracle for segmented unaligned streams.
`ifndef RTL002B_COUNT
`define RTL002B_COUNT 65537
`endif
module tb;
    localparam int N = `RTL002B_COUNT;
    localparam int HALF = N / 2;
    typedef logic [16:0] first_t [0:HALF-1];
    typedef logic [16:0] second_t [0:N-HALF-1];
    typedef logic [16:0] result_t [0:N-1];
    first_t first;
    second_t second;
    result_t result, pending;
    function automatic logic [16:0] expected(input longint unsigned cell_index);
        longint unsigned out_position, source_position, residual, cell_number;
        logic [16:0] source_value;
        logic [16:0] answer;
        residual = (64'(N) * 17) % 7;
        if (residual == 0) residual = 7;
        for (int bit_index = 0; bit_index < 17; bit_index++) begin
            out_position = cell_index * 17 + (16 - bit_index);
            if (out_position < residual) source_position = 64'(N) * 17 - residual + out_position;
            else source_position = 64'(N) * 17 - residual - ((out_position - residual) / 7 + 1) * 7 + (out_position - residual) % 7;
            cell_number = source_position / 17;
            if (cell_number < HALF) begin
                source_value = 17'h12345;
                if (cell_number == 0) source_value = 17'h1abcd;
            end else begin
                source_value = 17'h07654;
                if (cell_number == N - 1) source_value = 17'h05555;
            end
            answer[bit_index] = source_value[16 - source_position % 17];
        end
        return answer;
    endfunction
    initial begin
        first = '{default:17'h12345};
        second = '{default:17'h07654};
        first[0] = 17'h1abcd;
        second[N-HALF-1] = 17'h05555;
        result = {<<7{first, second}};
        if (result[0] !== expected(0) || result[1] !== expected(1) || result[HALF] !== expected(HALF) || result[N-2] !== expected(N-2) || result[N-1] !== expected(N-1)) $fatal;
        pending <= {<<7{first, second}};
        first[0] = 0;
        second[N-HALF-1] = 0;
        #1;
        if (pending[0] !== expected(0) || pending[N-1] !== expected(N-1)) $fatal;
        result = {>>{result}};
        if (result[HALF] !== expected(HALF)) $fatal;
        $display("PASS rtl002b streams");
        $finish(0);
    end
endmodule
