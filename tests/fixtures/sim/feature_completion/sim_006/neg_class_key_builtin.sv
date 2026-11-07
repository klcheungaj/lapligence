module tb; int a[semaphore]; initial begin semaphore s=new; a[s]=1; end endmodule
