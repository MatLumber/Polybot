import * as React from "react";
import { cn } from "../lib/utils";

export type CardProps = React.HTMLAttributes<HTMLDivElement>

export function Card({ className, ...props }: CardProps) {
  return (
    <div
      className={cn(
        "rounded-xl border bg-card/60 text-card-foreground shadow-2xl",
        "backdrop-blur-2xl backdrop-saturate-[1.8]",
        "border-white/15 hover:border-white/30 transition-all duration-300",
        "relative overflow-hidden group",
        "before:absolute before:inset-0 before:bg-gradient-to-br before:from-white/10 before:via-white/5 before:to-transparent before:opacity-0 before:group-hover:opacity-100 before:transition-all before:duration-500",
        "after:absolute after:inset-0 after:bg-gradient-to-tl after:from-primary/5 after:to-transparent after:opacity-50",
        "shadow-[0_8px_32px_0_rgba(0,0,0,0.2)] hover:shadow-[0_8px_48px_0_rgba(0,0,0,0.3)]",
        className
      )}
      {...props}
    />
  );
}

export function CardHeader({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("flex flex-col space-y-1 p-3", className)}
      {...props}
    />
  );
}

export function CardTitle({
  className,
  ...props
}: React.HTMLAttributes<HTMLHeadingElement>) {
  return (
    <h3
      className={cn(
        "text-sm font-semibold leading-none tracking-tight",
        className
      )}
      {...props}
    />
  );
}

export function CardDescription({
  className,
  ...props
}: React.HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p className={cn("text-xs text-muted-foreground", className)} {...props} />
  );
}

export function CardContent({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("p-3 pt-0", className)} {...props} />;
}

export function CardFooter({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div className={cn("flex items-center p-3 pt-0", className)} {...props} />
  );
}
