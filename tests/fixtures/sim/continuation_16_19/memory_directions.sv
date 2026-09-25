module tb;
    reg [7:0] omitted[-1:-3], started[-1:-3], explicit_range[-1:-3];
    integer i;
    initial begin
        for (i=-1; i>=-3; i=i-1) begin
            omitted[i]=0; started[i]=0; explicit_range[i]=0;
        end
        $readmemh("three.hex", omitted);
        $readmemh("two.hex", started, -2);
        $readmemh("three.hex", explicit_range, -1, -3);
        $display("omitted=%02h %02h %02h", omitted[-1], omitted[-2], omitted[-3]);
        $display("started=%02h %02h %02h", started[-1], started[-2], started[-3]);
        $display("explicit=%02h %02h %02h", explicit_range[-1], explicit_range[-2], explicit_range[-3]);
        $finish(0);
    end
endmodule
