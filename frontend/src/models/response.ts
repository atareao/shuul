export default interface Response<T> {
    status?: number;
    message?: string;
    data?: T | null;
}
