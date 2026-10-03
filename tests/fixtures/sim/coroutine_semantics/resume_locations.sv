module tb;
task automatic pause;
    #1;
    #2;
endtask
initial begin
    #3;
    pause();
end
endmodule
